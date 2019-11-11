use std::borrow::BorrowMut;
use std::io;
use std::slice::Chunks;

use super::error::{Error, ErrorCode, Result};
use super::value::Value;

pub struct Serializer<W> {
    writer: W,
}

pub trait IdentifyLast: Iterator + Sized {
    fn identify_last(self) -> Iter<Self>;
}

impl<It> IdentifyLast for It
where
    It: Iterator,
{
    fn identify_last(mut self) -> Iter<Self> {
        let e = self.next();
        Iter {
            iter: self,
            buffer: e,
        }
    }
}

pub struct Iter<It>
where
    It: Iterator,
{
    iter: It,
    buffer: Option<It::Item>,
}

impl<It> Iterator for Iter<It>
where
    It: Iterator,
{
    type Item = (bool, It::Item);

    fn next(&mut self) -> Option<Self::Item> {
        match self.buffer.take() {
            None => None,
            Some(e) => match self.iter.next() {
                None => Some((true, e)),
                Some(f) => {
                    self.buffer = Some(f);
                    Some((false, e))
                }
            },
        }
    }
}

impl<W: io::Write> Serializer<W> {
    pub fn new(writer: W) -> Self {
        Serializer { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    pub fn serialize_value(&mut self, value: &Value) -> Result<()> {
        match *value {
            Value::Int(i) => self.serialize_int(i),
            Value::Bytes(ref b) => self.serialize_binary(b),
            _ => Err(Error::SyntaxError(ErrorCode::UnknownType)),
        }
    }

    fn serialize_int(&mut self, v: i32) -> Result<()> {
        println!("{}", v);
        let bytes = match v {
            -16..=47 => vec![(0x90 + v) as u8],
            -2048..=2047 => vec![(((v >> 8) & 0xff) + 0xc8) as u8, (v & 0xff) as u8],
            -262144..=262143 => vec![
                (((v >> 16) & 0xff) + 0xd4) as u8,
                ((v >> 8) & 0xff) as u8,
                (v & 0xff) as u8,
            ],
            _ => vec![
                'I' as u8,
                (v >> 24 & 0xff) as u8,
                (v >> 16 & 0xff) as u8,
                (v >> 8 & 0xff) as u8,
                (v & 0xff) as u8,
            ],
        };

        self.writer.write_all(&bytes).map_err(From::from)
    }

    fn serialize_binary(&mut self, v: &[u8]) -> Result<()> {
        if v.len() < 16 {
            return self
                .writer
                .write(&[(v.len() - 0x20) as u8])
                .and_then(|_| self.writer.write_all(&v))
                .map_err(From::from);
        }
        for (last, chunk) in v.chunks(0xffff).identify_last() {
            let flag = if last { 'B' as u8 } else { 'b' as u8 };
            let len_bytes = (v.len() as u16).to_be_bytes();
            let res = self.writer.write_all(&[flag]).and_then(|_| {
                self.writer
                    .write_all(&len_bytes)
                    .and_then(|_| self.writer.write_all(chunk))
            });
            if let Err(e) = res {
                return Err(Error::IoError(e));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Serializer;
    use crate::value::Value;
    use crate::value::Value::Int;

    fn test_encode_ok(value: Value, target: &[u8]) {
        let mut ser = Serializer::new(Vec::new());
        assert!(ser.serialize_value(&value).is_ok());
        assert_eq!(ser.writer.to_vec(), target);
    }

    #[test]
    fn test_encode_int() {
        test_encode_ok(Int(0), &[0x90 as u8]);
        test_encode_ok(Int(-16), &[0x80]);
        test_encode_ok(Int(47), &[0xbf]);
        test_encode_ok(Int(48), &[0xc8, 0x30]);

        test_encode_ok(Int(-2048), &[0xc0, 0x00]);
        test_encode_ok(Int(-256), &[0xc7, 0x00]);
        test_encode_ok(Int(2047), &[0xcf, 0xff]);

        test_encode_ok(Int(-262144), &[0xd0, 0x00, 0x00]);
        test_encode_ok(Int(262143), &[0xd7, 0xff, 0xff]);
    }
}
