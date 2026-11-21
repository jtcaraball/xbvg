use embedded_hal::digital::OutputPin;
use rp2040_hal::gpio::{
    DynPinId, Error, FunctionSioOutput, Pin,
    PinState::{self, High, Low},
    PullDown,
};

pub type Segment = Pin<DynPinId, FunctionSioOutput, PullDown>;

pub struct Display<const S: usize> {
    mux_segment: [Segment; S],
    char_segment: [Segment; 8],
}

impl<const S: usize> Display<S> {
    pub fn new(mux: [Segment; S], char: [Segment; 8]) -> Self {
        Display {
            mux_segment: mux,
            char_segment: char,
        }
    }
}

#[cfg(not(feature = "common-cathode"))]
impl<const S: usize> Display<S> {
    pub const DEFAULT_MUX_STATE: PinState = Low;
    pub const DEFAULT_CHAR_STATE: PinState = High;

    #[inline]
    #[cfg(not(feature = "common-cathode"))]
    pub fn set_on(&mut self, i: usize) -> Result<(), Error> {
        self.mux_segment[i].set_high()
    }

    #[inline]
    #[cfg(not(feature = "common-cathode"))]
    pub fn set_off(&mut self, i: usize) -> Result<(), Error> {
        self.mux_segment[i].set_low()
    }

    #[inline]
    #[cfg(not(feature = "common-cathode"))]
    pub fn draw(&mut self, shape: u8) -> Result<(), Error> {
        let mut buf = shape;
        for pin in self.char_segment.as_mut_slice() {
            if buf & 0b0000_0001 == 1 {
                pin.set_low()?;
            } else {
                pin.set_high()?;
            }
            buf >>= 1;
        }
        Ok(())
    }
}

#[cfg(feature = "common-cathode")]
impl<const S: usize> Display<S> {
    pub const DEFAULT_MUX_STATE: PinState = High;
    pub const DEFAULT_CHAR_STATE: PinState = Low;

    #[inline]
    pub fn set_on(&mut self, i: usize) -> Result<(), Error> {
        self.mux_segment[i].set_low()
    }

    #[inline]
    pub fn set_off(&mut self, i: usize) -> Result<(), Error> {
        self.mux_segment[i].set_high()
    }

    #[inline]
    pub fn draw(&mut self, shape: u8) -> Result<(), Error> {
        let mut buf = shape;
        for pin in self.char_segment.as_mut_slice() {
            if buf & 0b0000_0001 == 1 {
                pin.set_high()?;
            } else {
                pin.set_low()?;
            }
            buf >>= 1;
        }
        Ok(())
    }
}

macro_rules! display {
    ($S:tt, $source:ident, [$($mux_pin:tt),*], [$($char_pin:tt),*]) => {
        Display::<$S>::new(
            [
                $($source.$mux_pin.into_push_pull_output_in_state(Display::<$S>::DEFAULT_MUX_STATE).into_dyn_pin()),*
            ],
            [
                $($source.$char_pin.into_push_pull_output_in_state(Display::<$S>::DEFAULT_CHAR_STATE).into_dyn_pin()),*
            ],
        )
    };
}

pub(crate) use display;
