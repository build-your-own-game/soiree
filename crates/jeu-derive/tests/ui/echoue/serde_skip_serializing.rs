// `#[serde(skip_serializing)]` est refusé : un décodage strict et sans ambiguïté (REM-60, PUN-91, SAL-119).
#[jeu::message]
pub struct M {
    pub a: u8,
    #[serde(skip_serializing)]
    pub b: Option<u8>,
}

fn main() {}
