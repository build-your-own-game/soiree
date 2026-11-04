// Un flottant nu n'est pas un `Message` : un geste porte un `Fini` (§ 2.2, HIS-45).
#[jeu::message]
pub struct Geste {
    pub x: f32,
}

fn main() {}
