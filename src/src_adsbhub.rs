//! Get aircraft informations from `ADSBHub` network with SBS formatting
//! See <http://woodair.net/sbs/article/barebones42_socket_data.htm> for SBS message specification
//! 

use crate::{internal_com, traffic_infos::{AddressType, TrafficInfos}};

use anyhow::{anyhow, Context};
use core::str;
use std::{io::Read, net::TcpStream, thread, time::Duration, str::FromStr};


const ADSBHUB_ADDR: &str = "data.adsbhub.org:5002";

// Subtype for SBS MSG messages
const SBS_MSG_TYPE_NONE: u32 = 0;
const SBS_MSG_TYPE_ES_IDENTIFICATION_AND_CATEGORY: u32 = 1;
const SBS_MSG_TYPE_ES_AIRBORNE_POSITION_MESSAGE: u32 = 3;
const SBS_MSG_TYPE_ES_AIRBORNE_VELOCITY_MESSAGE: u32 = 4;

// Position of data fields of an SBS MSG message
const SBS_FIELD_POS_MESSAGE_TYPE: usize = 0;
const SBS_FIELD_POS_TRANSMISSION_TYPE: usize = 1;
const SBS_FIELD_POS_HEX_IDENT: usize = 4;
const SBS_FIELD_POS_CALLSIGN: usize = 10;
const SBS_FIELD_POS_ALTITUDE: usize = 11;
const SBS_FIELD_POS_GROUND_SPEED: usize = 12;
const SBS_FIELD_POS_TRACK: usize = 13;
const SBS_FIELD_POS_LATITUDE: usize = 14;
const SBS_FIELD_POS_LONGITUDE: usize = 15;
const SBS_FIELD_POS_VERTICAL_RATE: usize = 16;
const SBS_FIELD_POS_IS_ON_GROUND: usize = 21;       // Last position

const BODY_FIELD_FIRST_POSITION: usize = SBS_FIELD_POS_CALLSIGN;   // Position of the first field of the message body


pub struct SrcAdsbhub {
    sender: internal_com::Sender,
}

impl SrcAdsbhub {
    /// Starts reception of Adsbhub traffic
    pub fn start_receive() {
        thread::spawn(|| {
            Self::work_thread();
        });
    }


    fn new() -> SrcAdsbhub {
        SrcAdsbhub {
            sender: internal_com::Sender::new(),
        }
    }


    fn work_thread() {
        let adsbhub = Self::new();
        loop {
            if let Err(e) = adsbhub.get_and_send_positions() {
                log::warn!("{:#}", e);
            }
            thread::sleep(Duration::from_secs(5));
        }
    }


    fn get_and_send_positions(&self) -> anyhow::Result<()> {
        // variable for get_message
        let mut rx_buf = [0u8; 100_000];
        let mut rx_buf_current_size = 0usize;
        let mut msg_begin_offset = 0usize;

        // variable for parse_message
        let mut traffic_infos = TrafficInfos::default();
        let mut last_message_type = SBS_MSG_TYPE_NONE;
        let mut last_hex_ident = 0u32;

        // Connection to ADSBHub network
        let mut sock = TcpStream::connect(ADSBHUB_ADDR).context("Failed to connect to ADSBHub")?;

        // Setting the socket to quickly detect a silent disconnection from the remote
        Self::set_sock_options(&sock);

        // Infinite message reading and processing loop
        loop {
            // Get one SBS message
            let msg = Self::get_message(&mut sock, &mut rx_buf, &mut rx_buf_current_size, &mut msg_begin_offset)?;
            //println!("SBS msg = {}", str::from_utf8(msg)?);

            // Parse the SBS message
            if let Some(()) = Self::parse_message(msg, &mut traffic_infos, &mut last_message_type, &mut last_hex_ident)? {
                // A complete sequence of messages has been received, the traffic information is valid
                //println!("{:?}", traffic_infos);

                // Sending traffic information to clients
                self.sender.send(&traffic_infos);
            }
        }
    }


    fn set_sock_options(socket: &TcpStream) {
        let sock = socket2::SockRef::from(socket);

        // Setting TCP keepalive
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(Duration::from_secs(30))
            .with_interval(Duration::from_secs(5))
            .with_retries(2);
        sock.set_tcp_keepalive(&keepalive).unwrap();
    }


    fn get_message<'a>(socket: &mut TcpStream, rx_buf: &'a mut [u8], rx_buf_current_size: &mut usize, begin_offset: &mut usize)
        -> anyhow::Result<&'a [u8]> {
        const MSG_END_VALUE: u8 = b'\n';
        let mut current_offset = *begin_offset;

        // Look for the end of the SBS message
        'msg_end_search: loop {
            // If we have processed all the data received, we retrieve new data
            if current_offset >= *rx_buf_current_size {
                // If there is no more space in the rx buffer,
                // we shift the message to the beginning of the buffer to be able to fill it
                if *rx_buf_current_size >= rx_buf.len() {
                    rx_buf.copy_within(*begin_offset..*rx_buf_current_size, 0usize);
                    *rx_buf_current_size -= *begin_offset;
                    *begin_offset = 0;
                    current_offset = *rx_buf_current_size;

                    // If the buffer is still full, it means that the message is larger than
                    // the reception buffer, this is not normal, we return an error
                    anyhow::ensure!(*rx_buf_current_size < rx_buf.len(), "SBS message too long");
                }

                // Retrieve new data
                let nb = socket.read(&mut rx_buf[*rx_buf_current_size..]).context("Failed to read data from ADSBHub")?;
                anyhow::ensure!(nb > 0, "Connection closed by ADSBHub");
                *rx_buf_current_size += nb;
            }

            // Look for the next end of message in the received data
            while current_offset < *rx_buf_current_size {
                // If we found the end of message, we leave the big loop
                if rx_buf[current_offset] == MSG_END_VALUE {
                    break 'msg_end_search;
                }
                current_offset += 1;
            }
        }

        // Found the end of message, we set the beginning of the next message
        let current_begin_offset = *begin_offset;
        *begin_offset = current_offset + 1;

        Ok(&rx_buf[current_begin_offset..current_offset])
    }


    fn parse_message(message: &[u8], traffic_infos: &mut TrafficInfos, last_message_type: &mut u32, last_hex_ident: &mut u32)
        -> anyhow::Result<Option<()>> {
        let msg = str::from_utf8(message).context("Invalid character in ADSBHub message")?;

        // Browses all message fields
        let mut last_position = 0;
        for (i, field) in msg.split(',').enumerate() {
            last_position = i;
            if i < BODY_FIELD_FIRST_POSITION {
                // Field of message header
                Self::parse_message_header_field(i, field, traffic_infos, last_message_type, last_hex_ident)?;
            }
            else {
                // Field of message body
                if *last_message_type == SBS_MSG_TYPE_ES_IDENTIFICATION_AND_CATEGORY {
                    Self::parse_message1_body_field(i, field, traffic_infos)?;
                }
                else if *last_message_type == SBS_MSG_TYPE_ES_AIRBORNE_POSITION_MESSAGE {
                    Self::parse_message3_body_field(i, field, traffic_infos)?;
                }
                else if *last_message_type == SBS_MSG_TYPE_ES_AIRBORNE_VELOCITY_MESSAGE {
                    Self::parse_message4_body_field(i, field, traffic_infos)?;
                }
                else {
                    return Err(anyhow!("Unexpected SBS MSG type"));
                }
            }
        }

        anyhow::ensure!(last_position == SBS_FIELD_POS_IS_ON_GROUND, "Wrong number of fields in SBS message");

        // If it is the last message in the sequence, we return that parse is completed
        if *last_message_type == SBS_MSG_TYPE_ES_AIRBORNE_VELOCITY_MESSAGE {
            Ok(Some(()))
        }
        else {
            Ok(None)
        }
    }


    fn parse_message_header_field(field_position: usize, field: &str, traffic_infos: &mut TrafficInfos, last_message_type: &mut u32, last_hex_ident: &mut u32)
        -> anyhow::Result<()> {
        match field_position {
            SBS_FIELD_POS_MESSAGE_TYPE => {         // The first field is constant
                anyhow::ensure!(field == "MSG", "First SBS field is not 'MSG'");
            },

            SBS_FIELD_POS_TRANSMISSION_TYPE => {    // We check the sequence of messages : ES_IDENTIFICATION_AND_CATEGORY then ES_AIRBORNE_POSITION_MESSAGE then ES_AIRBORNE_VELOCITY_MESSAGE
                let msg_type = u32::from_str(field).context("Failed to parse SBS MSG subtype")?;

                if msg_type == SBS_MSG_TYPE_ES_IDENTIFICATION_AND_CATEGORY {
                    if (*last_message_type == SBS_MSG_TYPE_NONE) || (*last_message_type == SBS_MSG_TYPE_ES_AIRBORNE_VELOCITY_MESSAGE) {
                        // First message in the sequence, we reset the information structure
                        *traffic_infos = TrafficInfos::default();
                    }
                    else {
                        return Err(anyhow!("Invalid SBS MSG sequence"));
                    }
                }
                else if msg_type == SBS_MSG_TYPE_ES_AIRBORNE_POSITION_MESSAGE {
                    anyhow::ensure!(*last_message_type == SBS_MSG_TYPE_ES_IDENTIFICATION_AND_CATEGORY, "Invalid SBS MSG sequence");
                }
                else if msg_type == SBS_MSG_TYPE_ES_AIRBORNE_VELOCITY_MESSAGE {
                    anyhow::ensure!(*last_message_type == SBS_MSG_TYPE_ES_AIRBORNE_POSITION_MESSAGE, "Invalid SBS MSG sequence");
                }
                else {
                    return Err(anyhow!("Unexpected SBS MSG subtype {}", msg_type));
                }

                *last_message_type = msg_type;
            },

            SBS_FIELD_POS_HEX_IDENT => {            // Hex identifier of the mode S transponder
                let hex_ident = u32::from_str_radix(field, 16).context("Failed to parse SBS MSG hex identifier")?;

                if *last_message_type == SBS_MSG_TYPE_ES_IDENTIFICATION_AND_CATEGORY {
                    // For the first message of the sequence, the identifier must be different from the previous one
                    anyhow::ensure!(hex_ident != *last_hex_ident, "SBS hex identifier must be different from last sequence");
                }
                else {
                    // For other messages, we must keep the same identifier
                    anyhow::ensure!(hex_ident == *last_hex_ident, "SBS hex identifier must be the same for all messages of the sequence");
                    traffic_infos.addr_type = AddressType::AdsbIcao;
                    traffic_infos.address = hex_ident;
                }

                *last_hex_ident = hex_ident;
            },

            _ => ()                                 // Other fields are not used and are therefore considered valid
        }

        Ok(())
    }

    
    fn parse_message1_body_field(field_position: usize, field: &str, traffic_infos: &mut TrafficInfos)
        -> anyhow::Result<()> {
        if field_position == SBS_FIELD_POS_CALLSIGN {
            anyhow::ensure!(!field.is_empty(), "Callsign in SBS message is empty");
            traffic_infos.callsign = String::from(field);
        }

        Ok(())
    }


    fn parse_message3_body_field(field_position: usize, field: &str, traffic_infos: &mut TrafficInfos)
        -> anyhow::Result<()> {
            match field_position {
                SBS_FIELD_POS_ALTITUDE => traffic_infos.altitude = i32::from_str(field).context("Failed to parse SBS MSG altitude")?,
                SBS_FIELD_POS_LATITUDE => traffic_infos.latitude = f64::from_str(field).context("Failed to parse SBS MSG latitude")?,
                SBS_FIELD_POS_LONGITUDE => traffic_infos.longitude = f64::from_str(field).context("Failed to parse SBS MSG longitude")?,
                _ => ()                                 // Other fields are not used and are therefore considered valid
            }
    
            Ok(())
        }


    fn parse_message4_body_field(field_position: usize, field: &str, traffic_infos: &mut TrafficInfos)
        -> anyhow::Result<()> {
            match field_position {
                SBS_FIELD_POS_GROUND_SPEED => {
                    traffic_infos.ground_speed = if field.is_empty() { None } else { Some(f64::from_str(field).context("Failed to parse SBS MSG ground speed")? as i32) };
                },

                SBS_FIELD_POS_TRACK => {
                    if field.is_empty() {
                        traffic_infos.track = None;
                    }
                    else {
                        let track = f64::from_str(field).context("Failed to parse SBS MSG track")?;
                        anyhow::ensure!((0.0..=360.0).contains(&track), "SBS MSG track out of range");
                        traffic_infos.track = Some(track as u32);
                    }
                },

                SBS_FIELD_POS_VERTICAL_RATE => {
                    traffic_infos.vertical_speed = if field.is_empty() { None } else { Some(i32::from_str(field).context("Failed to parse SBS MSG vertical rate")?) };
                },

                _ => ()                                 // Other fields are not used and are therefore considered valid
            }
    
            Ok(())
    }

}