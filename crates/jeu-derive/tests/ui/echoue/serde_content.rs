// `#[serde(content)]` est refusé : un décodage strict et sans ambiguïté (REM-60, PUN-91, SAL-119).
#[jeu::message]
#[serde(content = "c")]
pub enum M {
    A(u8),
    B(u16),
}

fn main() {}
