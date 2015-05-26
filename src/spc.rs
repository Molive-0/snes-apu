use binary_reader::{ReadAll, BinaryRead, BinaryReader};

use std::char;
use std::io::{Result, Error, ErrorKind, Seek, SeekFrom, BufReader};
use std::path::Path;
use std::fs::File;

pub struct Spc {
    pub header: [u8; 33],
    pub version_minor: u8,
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub psw: u8,
    pub sp: u8,
    pub id666_tag: Option<Id666>,
    pub ram: [u8; 0x10000],
    pub regs: [u8; 128],
    pub ipl_rom: [u8; 64]
}

impl Spc {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Spc> {
        macro_rules! bad_header {
            ($add_info:expr) => ({
                let message_text = "Unrecognized SPC header".to_string();
                let message =
                    match $add_info.len() {
                        0 => message_text,
                        _ => format!("{} ({})", message_text, $add_info)
                    };
                return Err(Error::new(ErrorKind::Other, message));
            });
            () => (bad_header!(""))
        }

        macro_rules! assert_header {    
            ($cond:expr, $message:expr) => (if !$cond { bad_header!($message); });
            ($cond:expr) => (assert_header!($cond, ""))
        }
        
        let file = try!(File::open(path));
        let mut r = BinaryReader::new(BufReader::new(file));

        let mut header = [0; 33];
        try!(r.read_all(&mut header));
        assert_header!(
            header.iter()
                .zip(b"SNES-SPC700 Sound File Data v0.30".iter())
                .all(|(x, y)| x == y),
            "Invalid header string");

        assert_header!(try!(r.read_le_u16()) == 0x1a1a);

        let has_id666_tag = match try!(r.read_u8()) {
            0x1a => true,
            0x1b => false,
            _ => bad_header!("Unable to determine if file contains ID666 tag")
        };

        let version_minor = try!(r.read_u8());

        let pc = try!(r.read_le_u16());
        let a = try!(r.read_u8());
        let x = try!(r.read_u8());
        let y = try!(r.read_u8());
        let psw = try!(r.read_u8());
        let sp = try!(r.read_u8());

        let id666_tag = match has_id666_tag {
            true => {
                try!(r.seek(SeekFrom::Start(0x2e)));
                match Id666::load(&mut r) {
                    Ok(x) => Some(x),
                    Err(e) => bad_header!(format!("Invalid ID666 tag [{}]", e))
                }
            },
            false => None
        };

        try!(r.seek(SeekFrom::Start(0x100)));
        let mut ram = [0; 0x10000];
        try!(r.read_all(&mut ram));
        let mut regs = [0; 128];
        try!(r.read_all(&mut regs));
        try!(r.seek(SeekFrom::Start(0x101c0)));
        let mut ipl_rom = [0; 64];
        try!(r.read_all(&mut ipl_rom));
        
        Ok(Spc {
            header: header,
            version_minor: version_minor,
            pc: pc,
            a: a,
            x: x,
            y: y,
            psw: psw,
            sp: sp,
            id666_tag: id666_tag,
            ram: ram,
            regs: regs,
            ipl_rom: ipl_rom
        })
    }
}

pub struct Id666 {
    pub song_title: String,
    pub game_title: String,
    pub dumper_name: String,
    pub date_dumped: String,
    pub seconds_to_play_before_fading_out: i32,
    pub fade_out_length: i32,
    pub artist_name: String,
    pub default_channel_disables: u8,
    pub dumping_emulator: Emulator
}

pub enum Emulator {
    Unknown,
    ZSnes,
    Snes9x
}

impl Id666 {
    fn load<R: BinaryRead + Seek>(r: &mut R) -> Result<Id666> {
        let song_title = try!(Id666::read_string(r, 32));
        let game_title = try!(Id666::read_string(r, 32));
        let dumper_name = try!(Id666::read_string(r, 16));
        let comments = try!(Id666::read_string(r, 32));

        println!("song title: [{}]", song_title);
        println!("game title: [{}]", game_title);
        println!("dumper name: [{}]", dumper_name);
        println!("comments: [{}]", comments);

        // So, apparently, there's really no reliable way to detect whether or not
        //  an id666 tag is in text or binary format. I tried using the date field,
        //  but that's actually invalid in a lot of files anyways. I've read that
        //  the dumping emu can give clues (znes seems to dump binary files and
        //  snes9x seems to dump text), but these don't cover cases where the
        //  dumping emu is "unknown", so that sucks too. I've even seen some source
        //  where people try to differentiate based on the value of the psw register
        //  (lol). Ultimately, the most sensible solution I was able to dig up that
        //  seems to work on all of the .spc's I've tried is to just check if there
        //  appears to be textual data where the length and/or date fields should be.
        //  Still pretty icky, but it works pretty well.
        try!(r.seek(SeekFrom::Start(0x9e)));
        let is_text_format = match try!(Id666::is_text_region(r, 11)) {
            true => {
                try!(r.seek(SeekFrom::Start(0xa9)));
                try!(Id666::is_text_region(r, 3))
            },
            _ => false
        };

        try!(r.seek(SeekFrom::Start(0x9e)));

        if is_text_format {
            // TODO: Find SPC's to test this with
            unimplemented!();
        } else {
            let year = try!(r.read_le_u16());
            let month = try!(r.read_u8());
            let day = try!(r.read_u8());
            let date_dumped = format!("{}/{}/{}", month, day, year);
            println!("date dumped: [{}]", date_dumped);

            try!(r.seek(SeekFrom::Start(0xa9)));
            let seconds_to_play_before_fading_out = try!(r.read_le_u16());
            println!("seconds to play before fading out: {}", seconds_to_play_before_fading_out);
            try!(r.read_u8());
            let fade_out_length = try!(r.read_le_i32());
            println!("fade out length: {}", fade_out_length);
        }

        let artist_name = try!(Id666::read_string(r, 32));
        println!("artis name: [{}]", artist_name);

        let default_channel_disables = try!(r.read_u8());

        let dumping_emulator = match try!(Id666::read_digit(r)) {
            1 => Emulator::ZSnes,
            2 => Emulator::Snes9x,
            _ => Emulator::Unknown
        };

        
        
        unimplemented!();
    }

    fn read_string<R: BinaryRead>(r: &mut R, max_len: i32) -> Result<String> {
        // TODO: Reimplement as iterator or something similar
        let mut ret = "".to_string();
        let mut has_ended = false;
        for _ in 0..max_len {
            let b = try!(r.read_u8());
            if !has_ended {
                match char::from_u32(b as u32) {
                    Some(c) => ret.push(c),
                    _ => has_ended = true
                }
            }
        }
        Ok(ret)
    }

    fn is_text_region<R: BinaryRead>(r: &mut R, len: i32) -> Result<bool> {
        for _ in 0..len {
            if let Some(c) = char::from_u32(try!(r.read_u8()) as u32) {
                if c != '/' && !c.is_alphabetic() && !c.is_digit(10) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn read_digit<R: BinaryRead>(r: &mut R) -> Result<i32> {
        // TODO: Remove debugging code
        let derp = char::from_u32(try!(r.read_u8()) as u32);
        println!("DERP: {:?}", derp);
        match derp {
            Some(c) if c.is_digit(10) => Ok(c.to_digit(10).unwrap() as i32),
            _ => Err(Error::new(ErrorKind::Other, "Expected numeric value"))
        }
    }
}
