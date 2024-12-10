use crate::traffic_infos::{AddressType, TrafficInfos};
use crate::internal_com;

use quick_xml::{events::Event, Reader};
use std::{thread, time, str::FromStr};


pub struct SrcOgn {
    sender: internal_com::Sender,
}

impl SrcOgn {
    /// Starts reception of OGN traffic
    pub fn start_receive() {
        thread::spawn(|| {
            Self::work_thread();
        });
    }


    fn new() -> SrcOgn {
        SrcOgn {
            sender: internal_com::Sender::new(),
        }
    }


    fn work_thread() {
        let ogn = Self::new();
        loop {
            if let Err(e) = ogn.get_and_send_positions() {
                log::warn!("{:?}", e);
            }
            thread::sleep(time::Duration::from_secs(5));
        }
    }


    fn get_and_send_positions(&self) -> anyhow::Result<()> {
        let ogn_string = Self::get_ogn_string()?;
        self.parse_ogn_string(&ogn_string)?;
        Ok(())
    }


    fn get_ogn_string() -> anyhow::Result<String> {
        // We retrieve traffic information on France
        let ogn_string = ureq::get("https://live.glidernet.org/lxml.php?\
            a=0\
            &b=51.3\
            &c=42.1\
            &d=8.4\
            &e=-5.1")
            .call()?
            .into_string()?;
        Ok(ogn_string)
    }


    fn parse_ogn_string(&self, ogn_string: &str) -> anyhow::Result<()> {
        // Parse the XML string with quick-xml
        let mut reader = Reader::from_str(ogn_string);
        loop {
            match reader.read_event()? {
                Event::Empty(element) => {
                    // OGN traffic is contained in empty XML elements with name "m"
                    if element.local_name().as_ref() == b"m" {
                        // Browse element attributes
                        for attribute in element.attributes() {
                            match attribute {
                                Err(e) => return Err(anyhow::anyhow!("Incorrect attribute : {}", e)),
                                Ok(attr) => {
                                    // The attribute containing the traffic information is "a"
                                    if attr.key.local_name().as_ref() == b"a" {
                                        // We recover its value
                                        let traffic_string = &(attr.unescape_value()?);

                                        // Analysis of the traffic chain
                                        let traffic_infos = Self::parse_traffic(traffic_string)?;
                                        //println!("{:?}", traffic_infos);
    
                                        // Sending traffic information to clients
                                        self.sender.send(&traffic_infos);
                                    }    
                                }
                            }
                        }
                    }
                },
                Event::Eof => break,    // End of the XML chain, we exit the loop
                _ => ()                 // Other events do not interest us
            }
        }

        Ok(())
    }


    fn parse_traffic(traffic_string: &str) -> anyhow::Result<TrafficInfos> {
        let mut traffic_infos = TrafficInfos::default();

        // Breaking down and parsing each field in the traffic chain
        let traffic_fields = traffic_string.split(',');
        for (i, traffic_field) in traffic_fields.enumerate() {
            match i {
                0 => traffic_infos.latitude = f64::from_str(traffic_field)?,
                1 => traffic_infos.longitude = f64::from_str(traffic_field)?,
                2 => traffic_infos.callsign = traffic_field.to_string(),
                4 => traffic_infos.altitude = Self::meter_to_feet(i32::from_str(traffic_field)?),
                7 => traffic_infos.track = Some(u32::from_str(traffic_field)?),
                8 => traffic_infos.ground_speed = Some(Self::kmh_to_kt(i32::from_str(traffic_field)?)),
                9 => traffic_infos.vertical_speed = Some(Self::mps_to_fpm(f64::from_str(traffic_field)?)),
                13 => {
                    let address = u32::from_str_radix(traffic_field, 16)?;
                    traffic_infos.address = address & 0x00ff_ffff; // We only keep the 24 least significant bits
                    traffic_infos.addr_type = AddressType::Ogn;
                },
                _ => () // The other fields are not used and are considered valid
            }
        }
        Ok(traffic_infos)
    }


    fn meter_to_feet(meter: i32) -> i32 {
        (f64::from(meter) * 3.28084) as i32
    }


    fn kmh_to_kt(kmh: i32) -> i32 {
        (f64::from(kmh) * 0.539_957) as i32
    }


    fn mps_to_fpm(mps: f64) -> i32{
        (mps * 196.850_394) as i32
    }

}