// `#[serde(deserialize_with)]` est refusé : un décodage strict et sans ambiguïté (REM-60, PUN-91, SAL-119).
#[jeu::message]
pub struct M {
    pub a: u8,
    #[serde(deserialize_with = "d")]
    pub b: Option<u8>,
}

fn main() {}
