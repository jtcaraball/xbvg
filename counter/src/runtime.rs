use crate::display::Display;
use core::cell::Cell;
use cortex_m::delay::Delay;
use critical_section::{Mutex};
use rp2040_flash::flash::{flash_range_erase, flash_range_program};

const XIP_BASE: usize = 0x10000000;
const XIP_OFFSET: usize = 0x1FC000;
const SECTOR_SIZE: usize = 4096;
const PAGE_SIZE: usize = 256;
const PAGES_IN_SECTOR: usize = SECTOR_SIZE / PAGE_SIZE;
const PI_FLAG: [u8; 4] = [0x24, 0x3F, 0x6A, 0x88]; // Used to detect our writes.
const PI_FLAG_VALUE: u32 = 2_288_664_356;

pub static GLOBAL_COUNTER: Mutex<Cell<Option<u32>>> = Mutex::new(Cell::new(None));
pub static NEXT_FREE: Mutex<Cell<Option<usize>>> = Mutex::new(Cell::new(None));

pub fn read_record(i: usize) -> Option<u32> {
    let p = (XIP_BASE + XIP_OFFSET + i * PAGE_SIZE) as *const u32;
    unsafe {
        let flag = core::ptr::read_volatile(p);
        let value = core::ptr::read_volatile(p.add(1));
        let check = core::ptr::read_volatile(p.add(2));
        (flag == PI_FLAG_VALUE && check == !value).then_some(value)
    }
}

pub fn reset_counter_in_flash() {
    critical_section::with(|cs| {
        GLOBAL_COUNTER.borrow(cs).set(None);
        unsafe {
            flash_range_erase(XIP_OFFSET as u32, SECTOR_SIZE as u32, true);
        }
    });
}

pub fn read_counter_from_flash() -> u32 {
    let mut next = 0usize;
    let mut counter = 0u32;

    while next < PAGES_IN_SECTOR {
        match read_record(next) {
            Some(v) => {
                counter = v;
                next += 1;
            }
            None => break,
        }
    }

    let full = next == PAGES_IN_SECTOR;
    critical_section::with(|cs| {
        GLOBAL_COUNTER.borrow(cs).set(Some(counter));
        if full {
            unsafe {
                flash_range_erase(XIP_OFFSET as u32, SECTOR_SIZE as u32, true);
            }
            next = 0;
        }
        NEXT_FREE.borrow(cs).set(Some(next));
    });

    if full {
        write_counter_to_flash();
    }
    counter
}

pub fn write_counter_to_flash() {
    critical_section::with(|cs| {
        let Some(value) = GLOBAL_COUNTER.borrow(cs).get() else {
            return;
        };
        let next_free = NEXT_FREE.borrow(cs);
        let page_num = next_free.get().unwrap_or_default();
        if page_num >= PAGES_IN_SECTOR {
            return;
        }
        let mut page = [0xFFu8; PAGE_SIZE];
        page[0..4].copy_from_slice(&PI_FLAG);
        page[4..8].copy_from_slice(&value.to_le_bytes());
        page[8..12].copy_from_slice(&(!value).to_le_bytes());
        unsafe {
            flash_range_program((XIP_OFFSET + page_num * PAGE_SIZE) as u32, &page, true);
        }
        next_free.set(Some(page_num + 1));
    });
}

pub const DOT: u8 = 0b1000_0000;

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Digit {
    Blank = 0b0000_0000,
    Zero = 0b0111_0111,
    One = 0b0010_0100,
    Two = 0b0101_1101,
    Three = 0b0110_1101,
    Four = 0b0010_1110,
    Five = 0b0110_1011,
    Six = 0b0111_1011,
    Seven = 0b0010_0101,
    Eight = 0b0111_1111,
    Nine = 0b0010_1111,
}

impl Digit {
    #[inline]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    pub fn from_number(v: u8) -> Digit {
        match v {
            0 => Digit::Zero,
            1 => Digit::One,
            2 => Digit::Two,
            3 => Digit::Three,
            4 => Digit::Four,
            5 => Digit::Five,
            6 => Digit::Six,
            7 => Digit::Seven,
            8 => Digit::Eight,
            9 => Digit::Nine,
            10..=u8::MAX => Digit::Blank,
        }
    }
}

pub struct Counter<const S: usize> {
    freq: u32,
    digits: [u8; S],
    display: Display<S>,
    delay: Delay,
}

impl<const S: usize> Counter<S> {
    const MUX_DELAY_MS: u32 = 1u32;

    #[inline]
    pub fn new(freq_ms: u32, display: Display<S>, delay: Delay) -> Counter<S> {
        const { assert!(S <= 32, "too many digits") }
        assert!(freq_ms >= Counter::<S>::MUX_DELAY_MS);

        let mut s = Counter {
            delay,
            display,
            digits: [0u8; S],
            freq: freq_ms / Counter::<S>::MUX_DELAY_MS,
        };
        s.load_counter();
        s
    }

    #[inline]
    fn load_counter(&mut self) {
        let mut value = read_counter_from_flash();
        for i in 1..S {
            self.digits[S - i] = (value % 10) as u8;
            value /= 10;
        }
    }

    #[inline]
    fn increment(&mut self) {
        critical_section::with(|cs| {
            let cc = GLOBAL_COUNTER.borrow(cs);
            cc.set(Some(cc.get().unwrap() + 1));
        });
        for i in 1..S {
            self.digits[S - i] = (self.digits[S - i] + 1) % 10;
            if self.digits[S - i] != 0 {
                return;
            }
        }
    }

    #[inline]
    fn print(&mut self) {
        for (i, digit) in self.digits.iter().enumerate() {
            let mut digit = Digit::from_number(*digit).to_u8();
            if (S - i - 1).is_multiple_of(3) && i != S - 1 {
                digit += DOT;
            }
            self.display.draw(digit);
            self.display.set_on(i);
            self.delay.delay_ms(Counter::<S>::MUX_DELAY_MS);
            self.display.set_off(i);
        }
    }

    #[inline]
    pub fn run(&mut self) -> ! {
        let mut i = 0u32;
        loop {
            self.print();
            i = (i + 1) % self.freq;
            if i == 0 {
                self.increment();
            }
        }
    }
}
