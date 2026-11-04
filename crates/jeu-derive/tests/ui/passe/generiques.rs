// Des génériques bornés, dans un message et dans un état (§ 8.2).
use jeu::prelude::*;

#[jeu::message]
pub struct Enveloppe<T: Message> {
    pub dedans: T,
    pub combien: u32,
}

#[jeu::etat]
pub struct Boite<T: Etat> {
    pub dedans: Vec<T>,
}

#[jeu::etat]
pub enum PeutEtre<T: Etat> {
    Rien,
    Quelque(T),
}

fn main() {
    let _ = Enveloppe { dedans: 3u8, combien: 1 };
    let _ = Boite { dedans: vec![PeutEtre::Quelque(1.5f64), PeutEtre::Rien] };
}
