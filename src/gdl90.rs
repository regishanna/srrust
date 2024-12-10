//! GDL90 message formatting
//! See <https://www.faa.gov/sites/faa.gov/files/air_traffic/technology/adsb/archival/GDL90_Public_ICD_RevA.PDF>
//! 

use crate::traffic_infos::{TrafficInfos, AddressType};


// Structure of a message

const HEAD_LEN: usize = 2;
const TAIL_LEN: usize = 3;
const FLAG_BYTE: u8 = 0x7e;
const CONTROL_ESCAPE_CHAR: u8 = 0x7d;


// TRAFFIC REPORT message

const TRAFFIC_REPORT_MESSAGE_ID: u8 = 20;
const TRAFFIC_REPORT_LEN: usize = 27;

const TRAFFIC_REPORT_ADDRESS_OFFSET: usize = 0;
const TRAFFIC_REPORT_LATITUDE_OFFSET: usize = 4;
const TRAFFIC_REPORT_LONGITUDE_OFFSET: usize = 7;
const TRAFFIC_REPORT_ALTITUDE_OFFSET: usize = 10;
const TRAFFIC_REPORT_MISC_INDICATOR_OFFSET: usize = 11;
const TRAFFIC_REPORT_HORIZONTAL_VELOCITY_OFFSET: usize = 13;
const TRAFFIC_REPORT_VERTICAL_VELOCITY_OFFSET: usize = 14;
const TRAFFIC_REPORT_TRACK_OFFSET: usize = 16;
const TRAFFIC_REPORT_CALLSIGN_OFFSET: usize = 18;


// CRC table
const CRC_ARRAY: [u16; 256] = [
    0x0000, 0x1021, 0x2042, 0x3063, 0x4084, 0x50A5, 0x60C6, 0x70E7,
    0x8108, 0x9129, 0xA14A, 0xB16B, 0xC18C, 0xD1AD, 0xE1CE, 0xF1EF,
    0x1231, 0x0210, 0x3273, 0x2252, 0x52B5, 0x4294, 0x72F7, 0x62D6,
    0x9339, 0x8318, 0xB37B, 0xA35A, 0xD3BD, 0xC39C, 0xF3FF, 0xE3DE,
    0x2462, 0x3443, 0x0420, 0x1401, 0x64E6, 0x74C7, 0x44A4, 0x5485,
    0xA56A, 0xB54B, 0x8528, 0x9509, 0xE5EE, 0xF5CF, 0xC5AC, 0xD58D,
    0x3653, 0x2672, 0x1611, 0x0630, 0x76D7, 0x66F6, 0x5695, 0x46B4,
    0xB75B, 0xA77A, 0x9719, 0x8738, 0xF7DF, 0xE7FE, 0xD79D, 0xC7BC,
    0x48C4, 0x58E5, 0x6886, 0x78A7, 0x0840, 0x1861, 0x2802, 0x3823,
    0xC9CC, 0xD9ED, 0xE98E, 0xF9AF, 0x8948, 0x9969, 0xA90A, 0xB92B,
    0x5AF5, 0x4AD4, 0x7AB7, 0x6A96, 0x1A71, 0x0A50, 0x3A33, 0x2A12,
    0xDBFD, 0xCBDC, 0xFBBF, 0xEB9E, 0x9B79, 0x8B58, 0xBB3B, 0xAB1A,
    0x6CA6, 0x7C87, 0x4CE4, 0x5CC5, 0x2C22, 0x3C03, 0x0C60, 0x1C41,
    0xEDAE, 0xFD8F, 0xCDEC, 0xDDCD, 0xAD2A, 0xBD0B, 0x8D68, 0x9D49,
    0x7E97, 0x6EB6, 0x5ED5, 0x4EF4, 0x3E13, 0x2E32, 0x1E51, 0x0E70,
    0xFF9F, 0xEFBE, 0xDFDD, 0xCFFC, 0xBF1B, 0xAF3A, 0x9F59, 0x8F78,
    0x9188, 0x81A9, 0xB1CA, 0xA1EB, 0xD10C, 0xC12D, 0xF14E, 0xE16F,
    0x1080, 0x00A1, 0x30C2, 0x20E3, 0x5004, 0x4025, 0x7046, 0x6067,
    0x83B9, 0x9398, 0xA3FB, 0xB3DA, 0xC33D, 0xD31C, 0xE37F, 0xF35E,
    0x02B1, 0x1290, 0x22F3, 0x32D2, 0x4235, 0x5214, 0x6277, 0x7256,
    0xB5EA, 0xA5CB, 0x95A8, 0x8589, 0xF56E, 0xE54F, 0xD52C, 0xC50D,
    0x34E2, 0x24C3, 0x14A0, 0x0481, 0x7466, 0x6447, 0x5424, 0x4405,
    0xA7DB, 0xB7FA, 0x8799, 0x97B8, 0xE75F, 0xF77E, 0xC71D, 0xD73C,
    0x26D3, 0x36F2, 0x0691, 0x16B0, 0x6657, 0x7676, 0x4615, 0x5634,
    0xD94C, 0xC96D, 0xF90E, 0xE92F, 0x99C8, 0x89E9, 0xB98A, 0xA9AB,
    0x5844, 0x4865, 0x7806, 0x6827, 0x18C0, 0x08E1, 0x3882, 0x28A3,
    0xCB7D, 0xDB5C, 0xEB3F, 0xFB1E, 0x8BF9, 0x9BD8, 0xABBB, 0xBB9A,
    0x4A75, 0x5A54, 0x6A37, 0x7A16, 0x0AF1, 0x1AD0, 0x2AB3, 0x3A92,
    0xFD2E, 0xED0F, 0xDD6C, 0xCD4D, 0xBDAA, 0xAD8B, 0x9DE8, 0x8DC9,
    0x7C26, 0x6C07, 0x5C64, 0x4C45, 0x3CA2, 0x2C83, 0x1CE0, 0x0CC1,
    0xEF1F, 0xFF3E, 0xCF5D, 0xDF7C, 0xAF9B, 0xBFBA, 0x8FD9, 0x9FF8,
    0x6E17, 0x7E36, 0x4E55, 0x5E74, 0x2E93, 0x3EB2, 0x0ED1, 0x1EF0
];


/// Formats a TRAFFIC REPORT message in a provided buffer
/// Returns the used size of the buffer
pub fn make_traffic_report_message(infos: &TrafficInfos, buffer: &mut [u8]) -> anyhow::Result<usize> {
    let mut buf = [0u8; HEAD_LEN + TRAFFIC_REPORT_LEN + TAIL_LEN];

    // Address
    {
        let offset = HEAD_LEN + TRAFFIC_REPORT_ADDRESS_OFFSET;
        buf[offset] = u8::from(&(infos.addr_type));
        buf[offset + 1] = ((infos.address >> 16) & 0xff) as u8;
        buf[offset + 2] = ((infos.address >> 8) & 0xff) as u8;
        buf[offset + 3] = (infos.address & 0xff) as u8;
    }

    // Latitude on 24 signed bits
    {
        let mut latitude = ((infos.latitude * f64::from(0x0080_0000)) / 180.0) as i32;
        latitude = latitude.clamp(-0x0040_0000, 0x003f_ffff);
        let offset = HEAD_LEN + TRAFFIC_REPORT_LATITUDE_OFFSET;
        buf[offset] = ((latitude >> 16) & 0xff) as u8;
        buf[offset + 1] = ((latitude >> 8) & 0xff) as u8;
        buf[offset + 2] = (latitude & 0xff) as u8;
    }

    // Longitude on 24 signed bits
    {
        let mut longitude = ((infos.longitude * f64::from(0x0080_0000)) / 180.0) as i32;
        longitude = longitude.clamp(-0x0080_0000, 0x007f_ffff);
        let offset = HEAD_LEN + TRAFFIC_REPORT_LONGITUDE_OFFSET;
        buf[offset] = ((longitude >> 16) & 0xff) as u8;
        buf[offset + 1] = ((longitude >> 8) & 0xff) as u8;
        buf[offset + 2] = (longitude & 0xff) as u8;
    }

    // Altitude on 12 bits, 1000 ft offset
    {
        let mut altitude = ((if infos.altitude < -1000 { -1000 } else {infos.altitude}) + 1000) / 25;
        if altitude > 0xffe {
            altitude = 0xffe;
        }
        let offset = HEAD_LEN + TRAFFIC_REPORT_ALTITUDE_OFFSET;
        buf[offset] = ((altitude >> 4) & 0xff) as u8;
        buf[offset + 1] |= ((altitude << 4) & 0xf0) as u8;
    }

    // Miscellanous indicators
    {
        let misc_indicator = (if infos.track.is_some() { 1u8 } else { 0 }) | 8;
        let offset = HEAD_LEN + TRAFFIC_REPORT_MISC_INDICATOR_OFFSET;
        buf[offset] |= misc_indicator;
    }

    // Ground speed on 12 bits
    {
        let ground_speed = match infos.ground_speed {
            None => 0xfffu32,
            Some(gs) => {
                if gs < 0 {
                    0
                }
                else if gs > 0xffe {
                    0xffe
                }
                else {
                    gs as u32
                }
            }
        };
        let offset = HEAD_LEN + TRAFFIC_REPORT_HORIZONTAL_VELOCITY_OFFSET;
        buf[offset] = ((ground_speed >> 4) & 0xff) as u8;
        buf[offset + 1] |= ((ground_speed << 4) & 0xf0) as u8;
    }

    // Vertical speed on 12 bits
    {
        let vertical_speed = match infos.vertical_speed {
            None => -0x800i32,
            Some(vs) => vs.clamp(-32640, 32640) / 64
        };
        let offset = HEAD_LEN + TRAFFIC_REPORT_VERTICAL_VELOCITY_OFFSET;
        buf[offset] |= ((vertical_speed >> 8) & 0x0f) as u8;
        buf[offset + 1] = (vertical_speed & 0xff) as u8;
    }

    // Track on 8 bits
    {
        let mut track = match infos.track {
            None => 0,
            Some(tr) => (tr * 256) / 360
        };
        if track > 255 {
            track = 255;
        }
        let offset = HEAD_LEN + TRAFFIC_REPORT_TRACK_OFFSET;
        buf[offset] = track as u8;
    }

    // Callsign on 8 characters
    {
        let offset = HEAD_LEN + TRAFFIC_REPORT_CALLSIGN_OFFSET;
        let size_callsign = infos.callsign.chars().count();
        for i in 0usize..8 {
            let c_to_append = if i < size_callsign { infos.callsign.chars().nth(i).unwrap() } else { ' ' };
            buf[offset + 1] = if c_to_append.is_ascii() { c_to_append as u8 } else { b'?' };
        }
    }

    // Filling header and tail fields
    fill_header_and_tail(TRAFFIC_REPORT_MESSAGE_ID, &mut buf);

    // Application of byte-stuffing
    byte_stuff(&buf, buffer)
}


impl From<&AddressType> for u8 {
    fn from(value: &AddressType) -> Self {
        match value {
            AddressType::AdsbIcao => 0,
            AddressType::Ogn => 6
        }
    }
}


fn fill_header_and_tail(message_id: u8, msg_buf: &mut [u8]) {
    // Flag byte at the beginning and at the end
    msg_buf[0] = FLAG_BYTE;
    msg_buf[msg_buf.len() - 1] = FLAG_BYTE;

    // Message id
    msg_buf[1] = message_id;

    // CRC on the message id and message data fields
    let crc = compute_crc(&msg_buf[1..(msg_buf.len() - 3)]);
    msg_buf[msg_buf.len() - 3] = (crc & 0xff) as u8;        // LSB first
    msg_buf[msg_buf.len() - 2] = ((crc >> 8) & 0xff) as u8;
}


/// CRC CRC-CCITT
/// The following buffer: 0x00 0x81 0x41 0xDB 0xD0 0x08 0x02
/// gives a CRC of 0x8BB3
fn compute_crc(buffer: &[u8]) -> u16 {
    let mut crc = 0u16;

    for val in buffer {
        crc = CRC_ARRAY[(crc >> 8) as usize] ^ (crc << 8) ^ u16::from(*val);
    }

    crc
}


fn byte_stuff(message: &[u8], buffer: &mut [u8]) -> anyhow::Result<usize> {
    let mut cur_len = 0usize;

    for (i, &val) in message.iter().enumerate() {
        if (i > 0) && (i < (message.len() - 1)) &&  // Start and end flags are excluded from replacement
            ((val == FLAG_BYTE) || (val == CONTROL_ESCAPE_CHAR)) {
            // Inserting a control escape character
            anyhow::ensure!(buffer.len() >= cur_len + 2, "Insufficient buffer size");  // Verification that the buffer size is large enough
            buffer[cur_len] = CONTROL_ESCAPE_CHAR;
            cur_len += 1;
            buffer[cur_len] = val ^ 0x20;
            cur_len += 1;
        }
        else {
            // No insert
            anyhow::ensure!(buffer.len() > cur_len, "Insufficient buffer size");  // Verification that the buffer size is large enough
            buffer[cur_len] = val;
            cur_len += 1;
        }
    }

    Ok(cur_len)
}
