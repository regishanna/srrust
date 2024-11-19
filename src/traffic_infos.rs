use serde::{Serialize, Deserialize};

/// Type d'adresse telle que definie par GDL90, champ "Address Type"
#[derive(Default, Debug, Serialize, Deserialize)]
pub enum AddressType {
    #[default]
    AdsbIcao,
    Ogn
}

/// Informations concernant un trafic
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct TrafficInfos {
    pub addr_type: AddressType,
    pub address: u32,                   // sur 24 bits
    pub callsign: String,
    pub altitude: i32,                  // en ft avec QNH de 1013 hPa
    pub latitude: f64,                  // en degres
    pub longitude: f64,                 // en degres
    pub track: Option<u32>,             // en degres
    pub ground_speed: Option<i32>,      // en kt
    pub vertical_speed: Option<i32>,    // en fpm
}
