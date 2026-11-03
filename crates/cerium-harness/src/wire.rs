//! Minimal Minecraft wire helpers for the test client and the capture decoder.
//!
//! This is deliberately separate from `cerium-protocol`, so the parity tests
//! never share code with the server they check. It only handles uncompressed,
//! unencrypted packets, which is all the status exchange uses.

use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

/// Largest frame the vanilla protocol allows (a 3-byte VarInt length).
const MAX_FRAME: u32 = 2_097_151;

/// Payload sent in the status ping. The server must echo it back unchanged.
pub const PING_PAYLOAD: i64 = 0x0123_4567_89ab_cdef;

pub fn write_varint(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// Reads a VarInt from the start of `bytes`. Returns the value and how many
/// bytes it used, or `None` if `bytes` doesn't hold a complete VarInt.
pub fn read_varint(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut value = 0u32;
    for (index, byte) in bytes.iter().take(5).enumerate() {
        value |= u32::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
    }
    None
}

pub fn write_string(out: &mut Vec<u8>, value: &str) {
    write_varint(out, value.len() as u32);
    out.extend_from_slice(value.as_bytes());
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

/// Sends one uncompressed packet: frame length, packet ID, body.
pub fn send_packet(stream: &mut impl Write, id: u32, body: &[u8]) -> io::Result<()> {
    let mut packet = Vec::new();
    write_varint(&mut packet, id);
    packet.extend_from_slice(body);
    let mut frame = Vec::new();
    write_varint(&mut frame, packet.len() as u32);
    frame.extend_from_slice(&packet);
    stream.write_all(&frame)
}

fn read_stream_varint(stream: &mut impl Read) -> io::Result<u32> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0u8];
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if let Some((value, _)) = read_varint(&bytes) {
            return Ok(value);
        }
        if bytes.len() == 5 {
            return Err(invalid("VarInt is too long"));
        }
    }
}

/// Reads one uncompressed packet and returns its ID and body.
pub fn read_packet(stream: &mut impl Read) -> io::Result<(u32, Vec<u8>)> {
    let length = read_stream_varint(stream)?;
    if length > MAX_FRAME {
        return Err(invalid("frame is too large"));
    }
    let mut packet = vec![0; length as usize];
    stream.read_exact(&mut packet)?;
    match read_varint(&packet) {
        Some((id, size)) => Ok((id, packet[size..].to_vec())),
        None => Err(invalid("missing packet ID")),
    }
}

/// What a server answered to a status ping.
pub struct StatusReply {
    /// The status JSON, exactly as sent.
    pub json: String,
    /// The pong packet: its ID and body.
    pub pong: (u32, Vec<u8>),
}

/// Performs a server-list ping: handshake, status request, then ping.
pub fn status_ping(address: SocketAddr, protocol: u32) -> io::Result<StatusReply> {
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;

    let mut handshake = Vec::new();
    write_varint(&mut handshake, protocol);
    write_string(&mut handshake, &address.ip().to_string());
    handshake.extend_from_slice(&address.port().to_be_bytes());
    write_varint(&mut handshake, 1);
    send_packet(&mut stream, 0x00, &handshake)?;
    send_packet(&mut stream, 0x00, &[])?;

    let (id, body) = read_packet(&mut stream)?;
    if id != 0x00 {
        return Err(invalid("expected a status response (packet 0x00)"));
    }
    let Some((length, size)) = read_varint(&body) else {
        return Err(invalid("status response has no string length"));
    };
    let end = size + length as usize;
    let Some(json) = body.get(size..end) else {
        return Err(invalid("status response string is truncated"));
    };
    let Ok(json) = String::from_utf8(json.to_vec()) else {
        return Err(invalid("status response is not UTF-8"));
    };

    send_packet(&mut stream, 0x01, &PING_PAYLOAD.to_be_bytes())?;
    let pong = read_packet(&mut stream)?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(StatusReply { json, pong })
}

/// Splits a captured byte stream into frames and describes each one.
pub fn describe_frames(stream: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest = stream;
    while !rest.is_empty() {
        let Some((length, size)) = read_varint(rest) else {
            lines.push(format!("<{} undecodable bytes>", rest.len()));
            break;
        };
        let end = size + length as usize;
        if end > rest.len() {
            lines.push(format!("<truncated frame: {} of {end} bytes>", rest.len()));
            break;
        }
        let id = match read_varint(&rest[size..end]) {
            Some((id, _)) => format!("0x{id:02x}"),
            None => "?".to_string(),
        };
        lines.push(format!("length {length}, packet {id}"));
        rest = &rest[end..];
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{describe_frames, read_packet, read_varint, send_packet, write_varint};

    #[test]
    fn varint_round_trips() {
        for value in [0, 1, 127, 128, 255, 25565, 2_097_151, u32::MAX] {
            let mut bytes = Vec::new();
            write_varint(&mut bytes, value);
            assert_eq!(read_varint(&bytes), Some((value, bytes.len())));
        }
    }

    #[test]
    fn varint_known_encodings() {
        let mut bytes = Vec::new();
        write_varint(&mut bytes, 300);
        assert_eq!(bytes, [0xac, 0x02]);
        assert_eq!(read_varint(&[0x80]), None);
    }

    #[test]
    fn packet_round_trips() {
        let mut wire = Vec::new();
        send_packet(&mut wire, 0x01, &[1, 2, 3]).unwrap();
        assert_eq!(wire, [4, 0x01, 1, 2, 3]);
        let (id, body) = read_packet(&mut wire.as_slice()).unwrap();
        assert_eq!((id, body), (0x01, vec![1, 2, 3]));
    }

    #[test]
    fn frames_are_described() {
        let stream = [2, 0x00, 9, 1, 0x01, 3, 0x02];
        let lines = describe_frames(&stream);
        assert_eq!(lines[0], "length 2, packet 0x00");
        assert_eq!(lines[1], "length 1, packet 0x01");
        assert_eq!(lines[2], "<truncated frame: 2 of 4 bytes>");
    }
}
