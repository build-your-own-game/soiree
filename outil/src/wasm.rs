//! Lancer un programme WebAssembly (`wasm32-wasip1`) et rendre ce qu'il
//! affiche, sans rien installer à côté de l'outil.
//!
//! Le programme n'a droit qu'à écrire sur sa sortie. Tout le reste du système
//! (l'horloge, le hasard, les fichiers, le réseau) l'arrête, avec un message
//! qui dit ce qu'il a demandé : une machine n'en a pas besoin.

use wasmi::{Caller, Engine, Error, Extern, ExternType, Linker, Memory, Module, Store};

/// Le module des fonctions que le système prête à un programme WASI.
const WASI: &str = "wasi_snapshot_preview1";
/// « Ce descripteur n'existe pas » : il n'y a ni fichier ni dossier ouvert.
const EBADF: i32 = 8;
/// Le plus qu'un programme peut afficher : au-delà, il tourne en rond.
const OUTPUT_MAX: usize = 64 * 1024 * 1024;

/// Les fonctions du système que la console prête au programme (voir [`lend`]).
const LENT: [&str; 8] = [
    "fd_write",
    "environ_sizes_get",
    "args_sizes_get",
    "environ_get",
    "args_get",
    "fd_prestat_get",
    "fd_prestat_dir_name",
    "proc_exit",
];

/// Ce que le programme a écrit.
#[derive(Default)]
struct Host {
    out: Vec<u8>,
    err: Vec<u8>,
}

/// Lance le programme et rend sa sortie standard.
pub fn run(bytes: &[u8]) -> Result<String, String> {
    let engine = Engine::default();
    let module = Module::new(&engine, bytes).map_err(|error| error.to_string())?;
    let mut linker: Linker<Host> = Linker::new(&engine);
    lend(&mut linker).map_err(|error| error.to_string())?;
    // Tout ce que le programme demande d'autre au système l'arrête.
    for import in module.imports() {
        let ExternType::Func(ty) = import.ty() else {
            continue;
        };
        if import.module() == WASI && LENT.contains(&import.name()) {
            continue;
        }
        let message = refusal(import.name());
        let _: &mut Linker<Host> = linker
            .func_new(
                import.module(),
                import.name(),
                ty.clone(),
                move |_, _, _| Err(Error::new(message.clone())),
            )
            .map_err(|error| error.to_string())?;
    }
    let mut store = Store::new(&engine, Host::default());
    let outcome = linker
        .instantiate_and_start(&mut store, &module)
        .and_then(|instance| instance.get_typed_func::<(), ()>(&store, "_start"))
        .and_then(|start| start.call(&mut store, ()));
    let host = store.into_data();
    match outcome {
        Ok(()) => Ok(String::from_utf8_lossy(&host.out).into_owned()),
        Err(error) if error.i32_exit_status() == Some(0) => {
            Ok(String::from_utf8_lossy(&host.out).into_owned())
        }
        Err(error) => {
            let said = String::from_utf8_lossy(&host.err);
            Err(format!("{error}\n{}", said.trim_end()))
        }
    }
}

/// Pourquoi le programme est arrêté quand il demande `name` au système.
fn refusal(name: &str) -> String {
    let what = if name.starts_with("clock_") {
        "lit l'horloge : le temps est une entrée de la machine"
    } else if name == "random_get" {
        "tire du hasard au système (souvent par une `HashMap`) : la graine est une entrée de la \
         machine, et une table se parcourt dans l'ordre avec `BTreeMap`"
    } else if name.starts_with("sock_") {
        "ouvre le réseau : la machine ne le connaît pas"
    } else if name.starts_with("path_") || name.starts_with("fd_") {
        "lit ou écrit un fichier : la machine ne connaît pas le disque"
    } else if name == "poll_oneoff" || name == "sched_yield" {
        "attend ou dort : la machine avance d'un pas quand on le lui demande"
    } else {
        "demande au système autre chose que d'écrire sa sortie"
    };
    format!("la machine {what} (`{name}`)")
}

/// La mémoire du programme.
fn memory(caller: &Caller<'_, Host>) -> Result<Memory, Error> {
    caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(|| Error::new("le programme n'a pas de mémoire"))
}

/// Une adresse du programme : 32 bits sans signe.
fn address(value: i32) -> Result<usize, Error> {
    usize::try_from(value.cast_unsigned()).map_err(|_| Error::new("adresse hors de la mémoire"))
}

fn read_u32(caller: &Caller<'_, Host>, at: usize) -> Result<u32, Error> {
    let mut bytes = [0_u8; 4];
    memory(caller)?
        .read(caller, at, &mut bytes)
        .map_err(|_| Error::new("lecture hors de la mémoire"))?;
    Ok(u32::from_le_bytes(bytes))
}

fn write_u32(caller: &mut Caller<'_, Host>, at: i32, value: u32) -> Result<(), Error> {
    let at = address(at)?;
    memory(caller)?
        .write(caller, at, &value.to_le_bytes())
        .map_err(|_| Error::new("écriture hors de la mémoire"))
}

/// Le peu que la console prête au programme : écrire, et s'arrêter.
fn lend(linker: &mut Linker<Host>) -> Result<(), wasmi::errors::LinkerError> {
    let _: &mut Linker<Host> = linker.func_wrap(
        WASI,
        "fd_write",
        |mut caller: Caller<'_, Host>, fd: i32, iovs: i32, count: i32, written: i32| {
            let mut bytes: Vec<u8> = Vec::new();
            let mut at = address(iovs)?;
            for _ in 0..count.cast_unsigned() {
                let start = usize::try_from(read_u32(&caller, at)?)
                    .map_err(|_| Error::new("adresse hors de la mémoire"))?;
                let length = usize::try_from(read_u32(&caller, at.saturating_add(4))?)
                    .map_err(|_| Error::new("longueur hors de la mémoire"))?;
                let mut part = vec![0_u8; length];
                memory(&caller)?
                    .read(&caller, start, &mut part)
                    .map_err(|_| Error::new("lecture hors de la mémoire"))?;
                bytes.extend_from_slice(&part);
                at = at.saturating_add(8);
            }
            let total = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
            let host = caller.data_mut();
            let sink = match fd {
                1 => &mut host.out,
                2 => &mut host.err,
                _ => return Ok(EBADF),
            };
            if sink.len().saturating_add(bytes.len()) > OUTPUT_MAX {
                return Err(Error::new("le programme affiche sans fin"));
            }
            sink.extend_from_slice(&bytes);
            write_u32(&mut caller, written, total)?;
            Ok(0)
        },
    )?;
    // Ni variables d'environnement, ni arguments : deux listes vides.
    for name in ["environ_sizes_get", "args_sizes_get"] {
        let _: &mut Linker<Host> = linker.func_wrap(
            WASI,
            name,
            |mut caller: Caller<'_, Host>, count: i32, size: i32| {
                write_u32(&mut caller, count, 0)?;
                write_u32(&mut caller, size, 0)?;
                Ok::<i32, Error>(0)
            },
        )?;
    }
    for name in ["environ_get", "args_get"] {
        let _: &mut Linker<Host> =
            linker.func_wrap(WASI, name, |_: Caller<'_, Host>, _: i32, _: i32| 0_i32)?;
    }
    // Aucun dossier n'est ouvert pour le programme.
    let _: &mut Linker<Host> = linker.func_wrap(
        WASI,
        "fd_prestat_get",
        |_: Caller<'_, Host>, _: i32, _: i32| EBADF,
    )?;
    let _: &mut Linker<Host> = linker.func_wrap(
        WASI,
        "fd_prestat_dir_name",
        |_: Caller<'_, Host>, _: i32, _: i32, _: i32| EBADF,
    )?;
    let _: &mut Linker<Host> =
        linker.func_wrap(WASI, "proc_exit", |_: Caller<'_, Host>, status: i32| {
            Err::<(), Error>(Error::i32_exit(status))
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_refusal_says_what_the_machine_asked_for() {
        assert!(refusal("clock_time_get").contains("horloge"));
        assert!(refusal("random_get").contains("BTreeMap"));
        assert!(refusal("path_open").contains("fichier"));
        assert!(refusal("sock_send").contains("réseau"));
    }
}
