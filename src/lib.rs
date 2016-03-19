use std::mem;
use std::fmt;
use std::error;
use std::cell::Cell;

///
/// #EncodingError
///
/// Returned by the Encoder when a value fails to encode.
///
#[derive(Debug)]
pub struct Error(String);

impl Error {
    pub fn new(msg: &str) -> Error {
        Error(msg.to_string())
    }

    pub fn out_of_bounds() -> Error {
        Error::new("Attempted to read out of bounds")
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        return &self.0;
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct Encoder {
    data: Vec<u8>,
    bool_index: usize,
    bool_shift: u8,
    last_error: Option<Error>,
}

impl Encoder {
    pub fn new() -> Encoder {
        Encoder {
            data: Vec::new(),
            bool_index: std::usize::MAX,
            bool_shift: 0,
            last_error: None,
        }
    }

    pub fn uint8(mut self, uint8: u8) -> Encoder {
        self.data.push(uint8);
        return self;
    }

    pub fn uint16(mut self, uint16: u16) -> Encoder {
        self.data.reserve(2);
        self.data.push((uint16 >> 8) as u8);
        self.data.push((uint16 & 0xFF) as u8);
        return self;
    }

    pub fn uint32(mut self, uint32: u32) -> Encoder {
        self.data.reserve(4);
        self.data.push((uint32 >> 24) as u8);
        self.data.push(((uint32 >> 16) & 0xFF) as u8);
        self.data.push(((uint32 >> 8) & 0xFF) as u8);
        self.data.push((uint32 & 0xFF) as u8);
        return self;
    }

    pub fn int8(self, int8: i8) -> Encoder {
        self.uint8(unsafe { mem::transmute_copy(&int8) })
    }

    pub fn int16(self, int16: i16) -> Encoder {
        self.uint16(unsafe { mem::transmute_copy(&int16) })
    }

    pub fn int32(self, int32: i32) -> Encoder {
        self.uint32(unsafe { mem::transmute_copy(&int32) })
    }

    pub fn float32(self, float32: f32) -> Encoder {
        self.uint32(unsafe { mem::transmute_copy(&float32) })
    }

    pub fn float64(self, float64: f64) -> Encoder {
        let uint64: u64 = unsafe { mem::transmute_copy(&float64) };
        return self
            .uint32((uint64 >> 32) as u32)
            .uint32((uint64 & 0xFFFFFFFF) as u32);
    }

    pub fn bool(mut self, bool: bool) -> Encoder {
        let bool_bit: u8 = if bool { 1 } else { 0 };
        let index = self.data.len();

        if self.bool_index == index && self.bool_shift < 7 {
            self.bool_shift += 1;
            self.data[index - 1] = self.data[index - 1] | bool_bit << self.bool_shift;
            return self;
        }

        self.bool_index = index + 1;
        self.bool_shift = 0;
        self.uint8(bool_bit)
    }

    pub fn size(mut self, size: usize) -> Encoder {
        if size > 0x3FFFFFFF {
            self.last_error = Some(Error::new("[size] value is too large"));
            return self;
        }

        // can fit on 7 bits
        if size < 0x80 {
            return self.uint8(size as u8);
        }

        // can fit on 14 bits
        if size < 0x4000 {
            return self.uint16((size as u16) | 0x8000);
        }

        // use up to 30 bits
        return self.uint32((size as u32) | 0xC0000000);
    }

    pub fn bytes(mut self, bytes: &[u8]) -> Encoder {
        let size = bytes.len();
        if size > 0x3FFFFFFF {
            self.last_error = Some(Error::new("[bytes] is too long"));
            return self;
        }
        let mut sref = self.size(size);
        sref.data.extend_from_slice(bytes);
        return sref;
    }

    pub fn string(mut self, string: &str) -> Encoder {
        let size = string.len();
        if size > 0x3FFFFFFF {
            self.last_error = Some(Error::new("[string] is too long"));
            return self;
        }
        let mut sref = self.size(size);
        sref.data.extend_from_slice(string.as_bytes());
        return sref;
    }

    pub fn end(self) -> Result<Vec<u8>, Error> {
        match self.last_error {
            Some(error) => Err(error),
            None                => Ok(self.data),
        }
    }
}

pub struct Decoder {
    index: Cell<usize>,
    length: usize,
    data: Vec<u8>,
    bool_index: Cell<usize>,
    bool_shift: Cell<u8>,
}

impl Decoder {
    pub fn new(data: Vec<u8>) -> Decoder {
        Decoder {
            index: Cell::new(0),
            length: data.len(),
            data: data,
            bool_index: Cell::new(std::usize::MAX),
            bool_shift: Cell::new(0),
        }
    }

    pub fn uint8(&self) -> Result<u8, Error> {
        let index = self.index.get();
        if index >= self.length {
            return Err(Error::out_of_bounds());
        }
        let uint8 = self.data[index];
        self.index.set(index + 1);
        return Ok(uint8);
    }

    pub fn uint16(&self) -> Result<u16, Error> {
        Ok(
            (try!(self.uint8()) as u16) << 8 |
            (try!(self.uint8()) as u16)
        )
    }

    pub fn uint32(&self) -> Result<u32, Error> {
        Ok(
            (try!(self.uint8()) as u32) << 24 |
            (try!(self.uint8()) as u32) << 16 |
            (try!(self.uint8()) as u32) << 8    |
            (try!(self.uint8()) as u32)
        )
    }

    pub fn int8(&self) -> Result<i8, Error> {
        let uint8 = try!(self.uint8());
        Ok(unsafe { mem::transmute_copy(&uint8) })
    }

    pub fn int16(&self) -> Result<i16, Error> {
        let uint16 = try!(self.uint16());
        Ok(unsafe { mem::transmute_copy(&uint16) })
    }

    pub fn int32(&self) -> Result<i32, Error> {
        let uint32 = try!(self.uint32());
        Ok(unsafe { mem::transmute_copy(&uint32) })
    }

    pub fn float32(&self) -> Result<f32, Error> {
        let uint32 = try!(self.uint32());
        Ok(unsafe { mem::transmute_copy(&uint32) })
    }

    pub fn float64(&self) -> Result<f64, Error> {
        let uint64 = (try!(self.uint32()) as u64) << 32 |
                                 (try!(self.uint32()) as u64);
        Ok(unsafe { mem::transmute_copy(&uint64) })
    }

    pub fn bool(&self) -> Result<bool, Error> {
        let index = self.index.get();
        let mut bool_shift = self.bool_shift.get();

        if self.bool_index.get() == index && bool_shift < 7 {
            bool_shift += 1;
            self.bool_shift.set(bool_shift);
            let bits = self.data[index - 1];
            let bool_bit = 1 << bool_shift;
            return Ok(bits & bool_bit == bool_bit);
        }

        let bits = try!(self.uint8());
        self.bool_index.set(self.index.get());
        self.bool_shift.set(0);
        Ok(bits & 1 == 1)
    }

    pub fn size(&self) -> Result<usize, Error> {
        let mut size: usize = try!(self.uint8()) as usize;

        // 1 byte (no signature)
        if (size & 128) == 0 {
            return Ok(size);
        }

        let sig: u8 = (size as u8) >> 6;
        // remove signature from the first byte
        size = size & 63 /* 00111111 */;

        // 2 bytes (signature is 10)
        if sig == 2 {
            return Ok(size << 8 | try!(self.uint8()) as usize);
        }

        Ok(
            size << 24                          |
            (try!(self.uint8()) as usize) << 16 |
            (try!(self.uint8()) as usize) << 8  |
            (try!(self.uint8()) as usize)
        )
    }

    pub fn bytes(&self) -> Result<Vec<u8>, Error> {
        let size = try!(self.size());
        let index = self.index.get();
        if index + size > self.length {
            return Err(Error::out_of_bounds());
        }

        let bytes = self.data[index .. index + size].to_vec();

        self.index.set(index + size);

        return Ok(bytes);
    }

    pub fn string(&self) -> Result<String, Error> {
        let bytes = try!(self.bytes());
        return match String::from_utf8(bytes) {
            Ok(string) => Ok(string),
            Err(_) => Err(Error::new("Couldn't decode UTF-8 string")),
        }
    }

    pub fn end(&self) -> bool {
        self.index.get() >= self.length
    }
}
