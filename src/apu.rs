use std::rc::Rc;

use super::dsp::dsp::Dsp;
use super::smp::Smp;
use super::spc::spc::{Spc, IPL_ROM_LEN, RAM_LEN};
use super::timer::Timer;

const DEFAULT_IPL_ROM: [u8; IPL_ROM_LEN] = [
    0xcd, 0xef, 0xbd, 0xe8, 0x00, 0xc6, 0x1d, 0xd0, 0xfc, 0x8f, 0xaa, 0xf4, 0x8f, 0xbb, 0xf5, 0x78,
    0xcc, 0xf4, 0xd0, 0xfb, 0x2f, 0x19, 0xeb, 0xf4, 0xd0, 0xfc, 0x7e, 0xf4, 0xd0, 0x0b, 0xe4, 0xf5,
    0xcb, 0xf4, 0xd7, 0x00, 0xfc, 0xd0, 0xf3, 0xab, 0x01, 0x10, 0xef, 0x7e, 0xf4, 0x10, 0xeb, 0xba,
    0xf6, 0xda, 0x00, 0xba, 0xf4, 0xc4, 0xf4, 0xdd, 0x5d, 0xd0, 0xdb, 0x1f, 0x00, 0x00, 0xc0, 0xff,
];

pub struct Apu<'a> {
    ram: Box<[u8; RAM_LEN]>,
    ipl_rom: &'a [u8; IPL_ROM_LEN],

    pub smp: Smp<'a>,
    pub dsp: Dsp<'a>,

    timers: [Timer; 3],

    is_ipl_rom_enabled: bool,
    dsp_reg_address: u8,
}

impl<'apu> Apu<'apu> {
    pub fn new() -> Rc<Apu<'apu>> {
        Rc::new_cyclic(|ptr| Apu {
            ram: Box::new([0; RAM_LEN]),
            ipl_rom: &DEFAULT_IPL_ROM,

            smp: Smp::new(ptr.clone()),
            dsp: Dsp::new(ptr.clone()),

            timers: [Timer::new(256), Timer::new(256), Timer::new(32)],

            is_ipl_rom_enabled: true,
            dsp_reg_address: 0,
        })
    }

    pub fn from_spc(spc: &Spc) -> Rc<Apu> {
        let mut ret = Apu::new();

        *ret.ram = spc.ram;

        *ret.ipl_rom = spc.ipl_rom;

        {
            ret.smp.reg_pc = spc.pc;
            ret.smp.reg_a = spc.a;
            ret.smp.reg_x = spc.x;
            ret.smp.reg_y = spc.y;
            ret.smp.set_psw(spc.psw);
            ret.smp.reg_sp = spc.sp;
        }

        ret.dsp.set_state(spc);

        for (i, timer) in ret.timers.iter_mut().enumerate() {
            let target = ret.ram[0xfa + i];
            timer.set_target(target);
        }
        let control_reg = ret.ram[0xf1];
        ret.set_control_reg(control_reg);

        ret.dsp_reg_address = ret.ram[0xf2];

        ret
    }

    pub fn render(&mut self, buffer: &mut [(i16, i16)]) {
        while self.dsp.output_buffer.len() < buffer.len() {
            self.smp.run(buffer.len() * 64);
            self.dsp.flush();
        }

        for (sample, out) in self.dsp.output_buffer.drain(..buffer.len()).zip(buffer) {
            *out = sample;
        }
    }

    pub fn cpu_cycles_callback(&mut self, num_cycles: usize) {
        self.dsp.cycles_callback(num_cycles);
        for timer in self.timers.iter_mut() {
            timer.cpu_cycles_callback(num_cycles);
        }
    }

    pub fn read_u8(&mut self, address: u16) -> u8 {
        match address {
            0xf0 | 0xf1 => 0,

            0xf2 => self.dsp_reg_address,
            0xf3 => self.dsp.get_register(self.dsp_reg_address),

            0xfa..=0xfc => 0,

            0xfd => self.timers[0].read_counter(),
            0xfe => self.timers[1].read_counter(),
            0xff => self.timers[2].read_counter(),

            addr if addr >= 0xffc0 && self.is_ipl_rom_enabled => {
                self.ipl_rom[(addr - 0xffc0) as usize]
            }

            _ => self.ram[address as usize],
        }
    }

    pub fn write_u8(&mut self, address: u16, value: u8) {
        match address {
            0xf0 => {
                self.set_test_reg(value);
            }
            0xf1 => {
                self.set_control_reg(value);
            }
            0xf2 => {
                self.dsp_reg_address = value;
            }
            0xf3 => {
                self.dsp.set_register(self.dsp_reg_address, value);
            }

            0xfa => {
                self.timers[0].set_target(value);
            }
            0xfb => {
                self.timers[1].set_target(value);
            }
            0xfc => {
                self.timers[2].set_target(value);
            }

            0xfd..=0xff => (), // Do nothing

            _ => self.ram[address as usize] = value,
        }
    }

    pub fn clear_echo_buffer(&mut self) {
        let length = self.dsp.calculate_echo_length();
        let mut end_addr = self.dsp.get_echo_start_address() as i32 + length;
        if end_addr > RAM_LEN as i32 {
            end_addr = RAM_LEN as i32;
        }
        for i in (self.dsp.get_echo_start_address() as i32)..end_addr {
            self.ram[i as usize] = 0xff;
        }
    }

    fn set_test_reg(&self, _value: u8) {
        unimplemented!("Test reg not yet implemented");
    }

    fn set_control_reg(&mut self, value: u8) {
        self.is_ipl_rom_enabled = (value & 0x80) != 0;
        if (value & 0x20) != 0 {
            self.write_u8(0xf6, 0x00);
            self.write_u8(0xf7, 0x00);
        }
        if (value & 0x10) != 0 {
            self.write_u8(0xf4, 0x00);
            self.write_u8(0xf5, 0x00);
        }
        self.timers[0].set_start_stop_bit((value & 0x01) != 0);
        self.timers[1].set_start_stop_bit((value & 0x02) != 0);
        self.timers[2].set_start_stop_bit((value & 0x04) != 0);
    }
}
