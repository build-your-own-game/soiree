// `Variantes` ne s'écrit que pour une enum sans champ (§ 2.2).
#[derive(jeu::Variantes)]
pub enum Phase {
    Debut,
    Milieu(u8),
}

fn main() {}
