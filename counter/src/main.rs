#![cfg_attr(not(test), no_std)]
#![no_main]

mod display;
mod runtime;

use core::cell::RefCell;

use crate::display::{Display, display};
use crate::runtime::{Counter, reset_counter_in_flash, write_counter_to_flash};
use cortex_m::delay::Delay;
use critical_section::Mutex;
use rp_pico::{
    Pins, XOSC_CRYSTAL_FREQ, entry, hal,
    pac::{self, interrupt},
};
use rp2040_hal::{
    Clock,
    gpio::{FunctionSioInput, Interrupt, Pin, PullDown, PullUp, bank0},
};

const DISPLAY_SIZE: usize = 12;
const FREQUENCY_MS: u32 = 10;

type PowerInterruptPin = Pin<bank0::Gpio28, FunctionSioInput, PullDown>;
type ResetInterruptPin = Pin<bank0::Gpio0, FunctionSioInput, PullUp>;
static POWER_IRQ_PIN: Mutex<RefCell<Option<PowerInterruptPin>>> = Mutex::new(RefCell::new(None));
static RESET_IRQ_PIN: Mutex<RefCell<Option<ResetInterruptPin>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let core = pac::CorePeripherals::take().unwrap();
    let delay = Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let sio = hal::Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let po_irq_pin = pins.gpio28.reconfigure();
    po_irq_pin.set_interrupt_enabled(Interrupt::EdgeLow, true);
    critical_section::with(|cs| {
        POWER_IRQ_PIN.borrow(cs).replace(Some(po_irq_pin));
    });

    let rs_irq_pin = pins.gpio0.reconfigure();
    rs_irq_pin.set_interrupt_enabled(Interrupt::EdgeLow, true);
    critical_section::with(|cs| {
        RESET_IRQ_PIN.borrow(cs).replace(Some(rs_irq_pin));
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);
    }

    let display = display!(
        DISPLAY_SIZE,
        pins,
        [
            gpio7, gpio6, gpio5, gpio4, gpio11, gpio10, gpio9, gpio8, gpio15, gpio14, gpio13,
            gpio12
        ],
        [
            gpio16, gpio17, gpio18, gpio19, gpio20, gpio21, gpio22, gpio26
        ]
    );

    let mut counter = Counter::<DISPLAY_SIZE>::new(FREQUENCY_MS, display, delay);
    counter.run();
}

#[allow(static_mut_refs)]
#[interrupt]
fn IO_IRQ_BANK0() {
    static mut PO_IRQ_PIN: Option<PowerInterruptPin> = None;
    static mut RS_IRQ_PIN: Option<ResetInterruptPin> = None;

    if PO_IRQ_PIN.is_none() {
        critical_section::with(|cs| {
            *PO_IRQ_PIN = POWER_IRQ_PIN.borrow(cs).take();
        });
    }

    if let Some(pin) = PO_IRQ_PIN
        && pin.interrupt_status(Interrupt::EdgeLow)
    {
        write_counter_to_flash();
        pin.clear_interrupt(Interrupt::EdgeLow);
        return;
    }

    if RS_IRQ_PIN.is_none() {
        critical_section::with(|cs| {
            *RS_IRQ_PIN = RESET_IRQ_PIN.borrow(cs).take();
        });
    }

    if let Some(pin) = RS_IRQ_PIN
        && pin.interrupt_status(Interrupt::EdgeLow)
    {
        reset_counter_in_flash();
        pin.clear_interrupt(Interrupt::EdgeLow);
    }
}

#[panic_handler]
#[inline(never)]
fn halt(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}
