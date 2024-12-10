use serde::{Serialize, Deserialize};

/// Address type as defined by GDL90, “Address Type” field
#[derive(Default, Debug, Serialize, Deserialize)]
pub enum AddressType {
    #[default]
    AdsbIcao,
    Ogn
}

/// Information regarding traffic
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct TrafficInfos {
    pub addr_type: AddressType,
    pub address: u32,                   // on 24 bits
    pub callsign: String,
    pub altitude: i32,                  // in ft with QNH of 1013 hPa
    pub latitude: f64,                  // in degrees
    pub longitude: f64,                 // in degrees
    pub track: Option<u32>,             // in degrees
    pub ground_speed: Option<i32>,      // in kt
    pub vertical_speed: Option<i32>,    // in fpm
}
