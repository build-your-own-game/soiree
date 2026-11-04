// `#[serde(other)]` est refusé : un décodage strict et sans ambiguïté (REM-60, PUN-91, SAL-119).
#[jeu::message]
pub enum M {
    A,
    #[serde(other)]
    B,
}

fn main() {}
