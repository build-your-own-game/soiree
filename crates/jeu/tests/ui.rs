//! Ce qui ne doit pas compiler, et ce qui doit (INTERFACE.md, § 8.3) :
//! `echeance_ne_nait_que_de_maintenant`, `maintenant_ne_se_garde_pas`,
//! `accord_declaration_types` (chaque ligne de la table de § 2.12, dans les deux sens),
//! `raison_creuse_refusee`, et un refus qui ne peut rien écrire.

#[test]
fn ce_qui_compile_et_ce_qui_ne_compile_pas() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/passe/*.rs");
    t.compile_fail("tests/ui/echoue/*.rs");
}
