use crate::traffic_infos::{AddressType, TrafficInfos};
use crate::internal_com;

use std::{thread, time, str::FromStr};

pub struct SrcOgn {
    sender: internal_com::Sender,
}

impl SrcOgn {
    /// Lance la reception des trafics OGN
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
        // On recupere les infos de trafic sur la france
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
        let traffic_beginning_pattern = "<m a=\"";
        let traffic_ending_pattern = "\"";

        let mut current_index = 0;
        loop {
            // Detection du debut d'un trafic par son pattern
            let traffic_beginning_index = ogn_string[current_index..].find(traffic_beginning_pattern);

            match traffic_beginning_index {
                None => break,          // Plus de chaines de trafic, on arrete l'analyse
                Some(v) => {
                    // On a trouve le pattern de debut, on cherche celui de fin
                    let traffic_string_start = current_index + v + traffic_beginning_pattern.len();
                    let traffic_string_end = traffic_string_start + ogn_string[traffic_string_start..]
                        .find(traffic_ending_pattern)
                        .ok_or(anyhow::anyhow!("Pattern de fin non trouve"))?;
                    current_index = traffic_string_end;

                    // Analyse de la chaine de trafic
                    let traffic_infos = Self::parse_traffic(&ogn_string[traffic_string_start..traffic_string_end])?;
                    //println!("{:?}", traffic_infos);

                    // Envoi de l'info de trafic aux clients
                    self.sender.send(&traffic_infos);
                }
            }
        }
        Ok(())
    }


    fn parse_traffic(traffic_string: &str) -> anyhow::Result<TrafficInfos> {
        let mut traffic_infos = TrafficInfos::new();

        // Decoupage et parse de chaque champ de la chaine de trafic
        let traffic_fields = traffic_string.split(",");
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
                    traffic_infos.address = address & 0xffffff; // On ne conserve que les 24 bits de poids faible
                    traffic_infos.addr_type = AddressType::Ogn;
                },
                _ => () // Les autres champs ne sont pas utilises et sont donc consideres valides
            }
        }
        Ok(traffic_infos)
    }


    fn meter_to_feet(meter: i32) -> i32 {
        ((meter as f64) * 3.28084) as i32
    }


    fn kmh_to_kt(kmh: i32) -> i32 {
        ((kmh as f64) * 0.539957) as i32
    }


    fn mps_to_fpm(mps: f64) -> i32{
        (mps * 196.850394) as i32
    }

}