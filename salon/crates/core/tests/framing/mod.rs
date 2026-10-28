//! Encode malformed game payloads through the real Host/Client packet
//! builders. The outer salon envelope stays valid: only its inner message
//! has a valid prefix followed by an extra byte.

use salon_core::{Caps, Client, ClientOutput, DeviceEvent, Host, Kind, LinkId, Ms, Output, Status};
use serde::{Serialize, Serializer, ser::SerializeTuple};

use super::net::{Down, Up, host_config, phone_config};

enum Payload<T> {
    Complete(T),
    Suffixed(T),
}

impl<T: Serialize> Serialize for Payload<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Complete(value) => value.serialize(serializer),
            Self::Suffixed(value) => {
                // Postcard tuples have no header, so this is exactly the
                // message's real encoding followed by one extra zero byte.
                let mut tuple = serializer.serialize_tuple(2)?;
                tuple.serialize_element(value)?;
                tuple.serialize_element(&0_u8)?;
                tuple.end()
            }
        }
    }
}

const NOW: Ms = Ms(1_000);
const LINK: LinkId = LinkId(1);
type HostUnderTest = Host<Up, Payload<Down>>;
type ClientUnderTest = Client<Payload<Up>, Down>;

/// The same output-driven exchange used by net, with different serializer
/// types at the sender. No packet schema or private decoder is copied here.
fn exchange(
    host: &mut HostUnderTest,
    client: &mut ClientUnderTest,
) -> (Vec<DeviceEvent<Up>>, Vec<ClientOutput<Down>>) {
    let mut events = Vec::new();
    let mut delivered = Vec::new();
    // Enough to drain Hello/Welcome, packets and acknowledgements. Each actor
    // already bounds its poll queue; the test also bounds the exchange loop.
    for _ in 0..32 {
        while let Some(output) = client.poll(NOW) {
            match output {
                ClientOutput::Connect => {
                    host.opened(NOW, LINK, Caps::DATAGRAM);
                    client.opened(NOW, Caps::DATAGRAM);
                }
                ClientOutput::Transmit(bytes) => host.received(NOW, LINK, &bytes),
                ClientOutput::Disconnect => host.closed(NOW, LINK),
                ClientOutput::Status(_)
                | ClientOutput::Reset
                | ClientOutput::Event(_)
                | ClientOutput::State(_) => delivered.push(output),
            }
        }
        while let Some(output) = host.poll(NOW) {
            match output {
                Output::Transmit { bytes, .. } => client.received(NOW, &bytes),
                Output::Close { .. } => client.closed(NOW),
                Output::Device(event) => events.push(event),
            }
        }
    }
    (events, delivered)
}

#[test]
fn host_rejects_suffixed_inner_events_and_states_then_accepts_complete_messages() {
    let mut host = HostUnderTest::new(NOW, host_config(1, Vec::new()));
    let mut client = ClientUnderTest::new(NOW, phone_config(1, Kind::Phone));
    exchange(&mut host, &mut client);
    assert_eq!(client.status(), Status::Connected);
    let device = client.device().unwrap();
    let answer = Up::Answer(41);
    let mut expected = postcard::to_allocvec(&answer).unwrap();
    expected.push(0);
    assert_eq!(
        postcard::to_allocvec(&Payload::Suffixed(answer)).unwrap(),
        expected,
    );
    client.send(&Payload::Suffixed(Up::Answer(41))).unwrap();
    client.set_state(&Payload::Suffixed(Up::Pad(1, 0))).unwrap();
    let (events, _) = exchange(&mut host, &mut client);
    assert!(
        events.is_empty(),
        "malformed payload reached the game: {events:?}"
    );

    client.send(&Payload::Complete(Up::Answer(42))).unwrap();
    client.set_state(&Payload::Complete(Up::Pad(0, 1))).unwrap();
    let (events, _) = exchange(&mut host, &mut client);
    assert_eq!(events.len(), 2, "valid payloads must still be delivered");
    assert!(events.iter().any(|event| matches!(event,
        DeviceEvent::Event { device: from, event: Up::Answer(42), .. } if *from == device)));
    assert!(events.iter().any(|event| matches!(event,
        DeviceEvent::State { device: from, state: Up::Pad(0, 1) } if *from == device)));
}

#[test]
fn client_rejects_suffixed_inner_events_and_states_then_accepts_complete_messages() {
    let mut host = HostUnderTest::new(NOW, host_config(1, Vec::new()));
    let mut client = ClientUnderTest::new(NOW, phone_config(1, Kind::Phone));
    exchange(&mut host, &mut client);
    assert_eq!(client.status(), Status::Connected);
    let device = client.device().unwrap();
    host.send(device, &Payload::Suffixed(Down::Note(41)))
        .unwrap();
    host.show(device, &Payload::Suffixed(Down::View(41, vec![7])))
        .unwrap();
    let (_, delivered) = exchange(&mut host, &mut client);
    assert!(
        delivered.is_empty(),
        "malformed payload reached the page: {delivered:?}"
    );

    host.send(device, &Payload::Complete(Down::Note(42)))
        .unwrap();
    host.show(device, &Payload::Complete(Down::View(42, vec![8])))
        .unwrap();
    let (_, delivered) = exchange(&mut host, &mut client);
    assert_eq!(delivered.len(), 2, "valid payloads must still be delivered");
    assert!(
        delivered
            .iter()
            .any(|output| matches!(output, ClientOutput::Event(Down::Note(42))))
    );
    assert!(delivered.iter().any(|output| matches!(output,
        ClientOutput::State(Down::View(42, contents)) if contents == &[8])));
}
