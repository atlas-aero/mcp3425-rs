//! A platform agnostic Rust driver for the MCP3425 (and newer variants
//! MCP3426/MCP3427/MCP3428 as well), based on the
//! [`embedded-hal`](https://github.com/rust-embedded/embedded-hal) traits.
//!
//! ## The Device
//!
//! The Microchip MCP3425 is a low-current 16-bit analog-to-digital converter.
//! The device has an I²C interface and an on-board ±2048mV reference. For more
//! information, see the
//! [datasheet](https://ww1.microchip.com/downloads/aemDocuments/documents/OTH/ProductDocuments/DataSheets/22072b.pdf).
//!
//! Variants [MCP3426/7/8](https://ww1.microchip.com/downloads/en/DeviceDoc/22226a.pdf)
//! are very similar, but support multiple input channels. They are supported as
//! well, but require to enable one of the following Cargo features:
//!
//! - `dual_channel` for MCP3426/7
//! - `quad_channel` for MCP3428
//!
//! ## Cargo Features
//!
//! The following feature flags exists:
//!
//! - `dual_channel` for dual-channel support (MCP3426/7/8)
//! - `quad_channel` for dual-channel support (MCP3428)
//! - `measurements`: Use the
//!   [measurements](https://github.com/thejpster/rust-measurements) crate
//!   to represent voltages instead of the custom
//!   [`Voltage`](https://docs.rs/mcp3425/*/mcp3425/struct.Voltage.html) wrapper
//!
//! ## Usage
//!
//! ### Instantiating
//!
//! Import this crate and an `embedded_hal` implementation (e.g.
//! [linux-embedded-hal](https://github.com/rust-embedded/linux-embedded-hal)).
//! Then instantiate the device in either
//! [`ContinuousMode`](struct.ContinuousMode.html) or
//! [`OneShotMode`](struct.OneShotMode.html):
//!
//! ```no_run
//! # extern crate linux_embedded_hal;
//! use linux_embedded_hal::{Delay, I2cdev};
//! use mcp3425::{MCP3425, Config, Resolution, Gain, Error, OneShotMode};
//!
//! # fn main() {
//! let dev = I2cdev::new("/dev/i2c-1").unwrap();
//! let address = 0x68;
//! let mut adc = MCP3425::new(dev, address, Delay, OneShotMode);
//! # }
//! ```
//!
//! (You can also use the shortcut functions
//! [`oneshot`](struct.MCP3425.html#method.oneshot) or
//! [`continuous`](struct.MCP3425.html#method.continuous) to create instances
//! of the [`MCP3425`](struct.MCP3425.html) type without having to specify the
//! type as parameter.)
//!
//! ### Configuration
//!
//! You can choose the conversion resolution / sample rate and the PGA gain
//! with a [`Config`](struct.Config.html) object.
//!
//! Use the methods starting with `with_` to create a (side-effect free) new
//! instance of the configuration where the specified setting has been
//! replaced.
//!
//! ```no_run
//! # use mcp3425::{Config, Resolution, Gain};
//! # fn main() {
//! use mcp3425::Channel;
//! let config = Config::default()
//!     .with_resolution(Resolution::Bits12Sps240)
//!     .with_gain(Gain::Gain1);
//! let high_res = config.with_resolution(Resolution::Bits16Sps15);
//! let high_gain = high_res.with_gain(Gain::Gain8);
//! # }
//! ```
//!
//! Note: If you enable the `dual_channel` or `quad_channel` Cargo features,
//! you can also use the method `.with_channel(...)` on the `Config` struct (if
//! your model supports multiple input channels).
//!
//! ### Measurements
//!
//! **One-Shot**
//!
//! You can trigger a one-shot measurement:
//!
//! ```no_run
//! # extern crate linux_embedded_hal;
//! # use linux_embedded_hal::{Delay, I2cdev};
//! # use mcp3425::{MCP3425, Config, Resolution, Gain, Error};
//! # fn main() {
//! # use mcp3425::Channel;
//! let dev = I2cdev::new("/dev/i2c-1").unwrap();
//! # let address = 0x68;
//! let mut adc = MCP3425::oneshot(dev, address, Delay);
//! let config = Config::default();
//! match adc.measure(&config) {
//!     Ok(voltage) => println!("ADC measured {} mV", voltage.as_millivolts()),
//!     Err(Error::I2c(e)) => println!("An I2C error happened: {}", e),
//!     Err(Error::VoltageTooHigh) => println!("Voltage is too high to measure"),
//!     Err(Error::VoltageTooLow) => println!("Voltage is too low to measure"),
//!     Err(Error::NotReady) => println!("Measurement not yet ready. This is a driver bug."),
//!     Err(Error::NotInitialized) => unreachable!(),
//! }
//! # }
//! ```
//!
//! As you can see, the saturation values are automatically converted to
//! proper errors.
//!
//! **Continuous**
//!
//! You can also configure the ADC in continuous mode:
//!
//! ```no_run
//! # extern crate linux_embedded_hal;
//! # use linux_embedded_hal::{Delay, I2cdev};
//! # use mcp3425::{MCP3425, Config, Resolution, Gain, Error};
//! # fn main() {
//! # use mcp3425::Channel;
//! let dev = I2cdev::new("/dev/i2c-1").unwrap();
//! # let address = 0x68;
//! let mut adc = MCP3425::continuous(dev, address, Delay);
//! let config = Config::default();
//! adc.set_config(&config).unwrap();
//! match adc.read_measurement() {
//!     Ok(voltage) => println!("ADC measured {} mV", voltage.as_millivolts()),
//!     Err(Error::I2c(e)) => println!("An I2C error happened: {}", e),
//!     Err(Error::VoltageTooHigh) => println!("Voltage is too high to measure"),
//!     Err(Error::VoltageTooLow) => println!("Voltage is too low to measure"),
//!     Err(Error::NotReady) => println!("Measurement not yet ready. Polling too fast?"),
//!     Err(Error::NotInitialized) => println!("You forgot to call .set_config"),
//! }
//! # }
//! ```

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

#[macro_use]
extern crate bitflags;

use byteorder::{BigEndian, ByteOrder};
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

#[cfg(feature = "measurements")]
extern crate measurements;
#[cfg(feature = "measurements")]
use measurements::voltage::Voltage;

/// All possible errors in this crate
#[derive(Debug)]
pub enum Error<E> {
    /// I2C bus error
    I2c(E),
    /// Voltage is too high to be measured.
    VoltageTooHigh,
    /// Voltage is too low to be measured.
    VoltageTooLow,
    /// A measurement in continuous mode has been triggered without previously
    /// writing the configuration to the device.
    NotInitialized,
    /// A measurement returned a stale result.
    ///
    /// In continuous mode, this can happen if you poll faster than the sample
    /// rate. See datasheet section 5.1.1 for more details.
    ///
    /// In one-shot mode, this is probably a timing bug that should be reported to
    /// <https://github.com/dbrgn/mcp3425-rs/issues/>!
    ///
    NotReady,
}

bitflags! {
    struct ConfigRegister: u8 {
        const NOT_READY = 0b10000000;
        const MODE = 0b00010000;
        const SAMPLE_RATE_H = 0b00001000;
        const SAMPLE_RATE_L = 0b00000100;
        const GAIN_H = 0b00000010;
        const GAIN_L = 0b00000001;
    }
}

impl ConfigRegister {
    fn is_ready(&self) -> bool {
        !self.contains(ConfigRegister::NOT_READY)
    }
}

/// ADC reference voltage: +-2048mV
const REF_MILLIVOLTS: i16 = 2048;

/// The two conversion mode structs implement this trait.
///
/// This allows the `MCP3425` instance to be generic over the conversion mode.
pub trait ConversionMode {
    /// Return the bitmask for this conversion mode
    fn bits(&self) -> u8;
}

/// Use the MCP3425 in One-Shot mode.
pub struct OneShotMode;

impl ConversionMode for OneShotMode {
    fn bits(&self) -> u8 {
        0b00000000
    }
}

/// Use the MCP3425 in Continuous Conversion mode.
pub struct ContinuousMode;

impl ConversionMode for ContinuousMode {
    fn bits(&self) -> u8 {
        0b00010000
    }
}

/// Conversion bit resolution and sample rate
///
/// * 15 SPS -> 16 bits
/// * 60 SPS -> 14 bits
/// * 240 SPS -> 12 bits
///
/// Defaults to 12 bits / 240 SPS (`Bits12Sps240`),
/// matching the power-on defaults of the device.
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum Resolution {
    /// 16 bits / 15 SPS. This allows you to measure voltage in 62.5 µV steps.
    Bits16Sps15 = 0b00001000,
    /// 14 bits / 60 SPS. This allows you to measure voltage in 250 µV steps.
    Bits14Sps60 = 0b00000100,
    /// 12 bits / 240 SPS. This allows you to measure voltage in 1 mV steps.
    Bits12Sps240 = 0b00000000,
}

impl Resolution {
    /// Return the bitmask for this sample rate.
    pub fn bits(&self) -> u8 {
        *self as u8
    }

    /// Return the number of bits of accuracy this sample rate gives you.
    pub fn res_bits(&self) -> u8 {
        match *self {
            Resolution::Bits16Sps15 => 16,
            Resolution::Bits14Sps60 => 14,
            Resolution::Bits12Sps240 => 12,
        }
    }

    /// Return the maximum output code.
    pub fn max(&self) -> i16 {
        match *self {
            Resolution::Bits16Sps15 => 32767,
            Resolution::Bits14Sps60 => 8191,
            Resolution::Bits12Sps240 => 2047,
        }
    }

    /// Return the minimum output code.
    pub fn min(&self) -> i16 {
        match *self {
            Resolution::Bits16Sps15 => -32768,
            Resolution::Bits14Sps60 => -8192,
            Resolution::Bits12Sps240 => -2048,
        }
    }
}

impl Default for Resolution {
    /// Default implementation matching the power-on defaults of the device.
    fn default() -> Self {
        Resolution::Bits12Sps240
    }
}

/// Programmable gain amplifier (PGA)
///
/// Defaults to no amplification (`Gain1`),
/// matching the power-on defaults of the device.
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum Gain {
    /// Amplification factor 1.
    Gain1 = 0b00000000,
    /// Amplification factor 2.
    Gain2 = 0b00000001,
    /// Amplification factor 4.
    Gain4 = 0b00000010,
    /// Amplification factor 8.
    Gain8 = 0b00000011,
}

impl Gain {
    /// Return the bitmask for this gain configuration.
    pub fn bits(&self) -> u8 {
        *self as u8
    }
}

impl Default for Gain {
    /// Default implementation matching the power-on defaults of the device.
    fn default() -> Self {
        Gain::Gain1
    }
}

/// Selected ADC channel
///
/// Defaults to channel 1.
#[derive(Copy, Clone, Debug)]
pub enum Channel {
    /// First channel (Default)
    Channel1 = 0b0000_0000,
    /// Second channel
    ///
    /// Note: Only supported by MCP3426/7/8, and if the `dual_channel` or
    /// `quad_channel` cargo feature is enabled.
    #[cfg(any(feature = "dual_channel", feature = "quad_channel", doc))]
    Channel2 = 0b0010_0000,
    /// Third channel
    ///
    /// Note: Only supported by MCP3428, and if the `quad_channel` cargo
    /// feature is enabled.
    #[cfg(any(feature = "quad_channel", doc))]
    Channel3 = 0b0100_0000,
    /// Fourth channel
    ///
    /// Note: Only supported by MCP3428, and if the `quad_channel` cargo
    /// feature is enabled.
    #[cfg(any(feature = "quad_channel", doc))]
    Channel4 = 0b0110_0000,
}

impl Default for Channel {
    fn default() -> Self {
        Self::Channel1
    }
}

impl Channel {
    /// Return the bitmask for this channel configuration.
    pub fn bits(&self) -> u8 {
        *self as u8
    }
}

/// Device configuration: Resolution, gain and input channel.
///
/// To instantiate this struct, use the `Default` implementation:
///
/// ```
/// # use mcp3425::{Config, Resolution, Gain};
/// let config = Config::default()
///     .with_resolution(Resolution::Bits14Sps60)
///     .with_gain(Gain::Gain2);
/// ```
///
/// Default values:
///
/// - Resolution: Bits12Sps240
/// - Gain: Gain1
/// - Channel: Channel1
///
/// Note: Creating and changing this instance does not have an immediate effect
/// on the device. It is only written when a measurement is triggered, or when
/// writing config explicitly with
/// [`set_config`](struct.MCP3425.html#method.set_config).
#[derive(Debug, Default, Copy, Clone)]
pub struct Config {
    /// Conversion bit resolution and sample rate.
    pub resolution: Resolution,
    /// Programmable gain amplifier (PGA).
    pub gain: Gain,
    /// Selected input channel
    pub channel: Channel,
}

impl Config {
    /// Create a new configuration where the resolution has been replaced
    /// with the specified value.
    pub fn with_resolution(&self, resolution: Resolution) -> Self {
        Config {
            resolution,
            gain: self.gain,
            channel: self.channel,
        }
    }

    /// Create a new configuration where the gain has been replaced
    /// with the specified value.
    pub fn with_gain(&self, gain: Gain) -> Self {
        Config {
            resolution: self.resolution,
            gain,
            channel: self.channel,
        }
    }

    /// Create a new configuration where the channel has been replaced
    /// with the specified value.
    #[cfg(any(feature = "dual_channel", feature = "quad_channel", doc))]
    pub fn with_channel(&self, channel: Channel) -> Self {
        Config {
            resolution: self.resolution,
            gain: self.gain,
            channel,
        }
    }

    /// Return the bitmask for the combined configuration values.
    fn bits(&self) -> u8 {
        self.channel.bits() | self.resolution.bits() | self.gain.bits()
    }
}

/// A voltage measurement.
#[cfg(not(feature = "measurements"))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Voltage {
    millivolts: i16,
}

#[cfg(not(feature = "measurements"))]
impl Voltage {
    /// Create a new `Voltage` instance from a millivolt measurement.
    pub fn from_millivolts(millivolts: i16) -> Self {
        Self { millivolts }
    }

    /// Return the voltage in millivolts.
    pub fn as_millivolts(&self) -> i16 {
        self.millivolts
    }

    /// Return the voltage in volts.
    pub fn as_volts(&self) -> f32 {
        self.millivolts as f32 / 1000.0
    }
}

/// Driver for the MCP3425 ADC
#[derive(Debug, Default)]
pub struct MCP3425<I2C, D, M> {
    /// The concrete I²C device implementation.
    i2c: I2C,
    /// The I²C device address.
    address: u8,
    /// The concrete Delay implementation.
    delay: D,
    /// The ADC conversion mode.
    mode: M,
    /// The configuration being used by the last measurement.
    config: Option<Config>,
}

impl<I2C, D, M> MCP3425<I2C, D, M>
where
    I2C: I2c,
    D: DelayNs,
    M: ConversionMode,
{
    /// Initialize the MCP3425 driver.
    ///
    /// This constructor is side-effect free, so it will not write any
    /// configuration to the device until a first measurement is triggered.
    pub fn new(i2c: I2C, address: u8, delay: D, mode: M) -> Self {
        MCP3425 {
            i2c,
            address,
            delay,
            mode,
            config: None,
        }
    }

    /// Read an i16 and the configuration register from the device.
    fn read_i16_and_config(&mut self) -> Result<(i16, ConfigRegister), Error<I2C::Error>> {
        let mut buf = [0, 0, 0];
        self.i2c.read(self.address, &mut buf).map_err(Error::I2c)?;
        let measurement = BigEndian::read_i16(&buf[0..2]);
        let config_reg = ConfigRegister::from_bits_truncate(buf[2]);
        Ok((measurement, config_reg))
    }

    /// Calculate the voltage for the measurement result at the specified sample rate.
    ///
    /// If the value is a saturation value, an error is returned.
    fn calculate_voltage(
        &self,
        measurement: i16,
        resolution: &Resolution,
    ) -> Result<Voltage, Error<I2C::Error>> {
        // Handle saturation / out of range values
        if measurement == resolution.max() {
            return Err(Error::VoltageTooHigh);
        } else if measurement == resolution.min() {
            return Err(Error::VoltageTooLow);
        }

        let converted =
            measurement as i32 * (REF_MILLIVOLTS * 2) as i32 / (1 << resolution.res_bits());
        // The "allow" annotation is needed because there are different Voltage
        // types, depending on the build flags.
        #[allow(clippy::useless_conversion)]
        Ok(Voltage::from_millivolts((converted as i16).into()))
    }

    /// Destroy the driver instance and return the I2C device.
    pub fn destroy(self) -> I2C {
        self.i2c
    }
}

impl<I2C, D> MCP3425<I2C, D, OneShotMode>
where
    I2C: I2c,
    D: DelayNs,
{
    /// Initialize the MCP3425 driver in One-Shot mode.
    ///
    /// This constructor is side-effect free, so it will not write any
    /// configuration to the device until a first measurement is triggered.
    pub fn oneshot(i2c: I2C, address: u8, delay: D) -> Self {
        MCP3425 {
            i2c,
            address,
            delay,
            mode: OneShotMode,
            config: None,
        }
    }

    /// Change the conversion mode to continuous.
    ///
    /// This conversion is side-effect free, so it will not write any
    /// configuration to the device until
    /// [`set_config`](struct.MCP3425.html#method.set_config) is called.
    pub fn into_continuous(self) -> MCP3425<I2C, D, ContinuousMode> {
        MCP3425::continuous(self.i2c, self.address, self.delay)
    }

    /// Do a one-shot voltage measurement.
    ///
    /// Return the result in millivolts.
    pub fn measure(&mut self, config: &Config) -> Result<Voltage, Error<I2C::Error>> {
        let command = ConfigRegister::NOT_READY.bits() | self.mode.bits() | config.bits();

        // Send command
        self.i2c
            .write(self.address, &[command])
            .map_err(Error::I2c)?;

        // Determine time to wait for the conversion to finish.
        // Values found by experimentation, these do not seem to be specified
        // in the datasheet.
        let sleep_ms = match config.resolution {
            Resolution::Bits12Sps240 => 4,
            Resolution::Bits14Sps60 => 15,
            Resolution::Bits16Sps15 => 57,
        };
        self.delay.delay_ms(sleep_ms + 2); // Add two additional milliseconds as safety margin

        // Read result
        let (measurement, config_reg) = self.read_i16_and_config()?;

        // Make sure that the delay was sufficient
        if !config_reg.is_ready() {
            return Err(Error::NotReady);
        }

        // Calculate voltage from raw value
        let voltage = self.calculate_voltage(measurement, &config.resolution)?;

        Ok(voltage)
    }
}

impl<I2C, D> MCP3425<I2C, D, ContinuousMode>
where
    I2C: I2c,
    D: DelayNs,
{
    /// Initialize the MCP3425 driver in Continuous Measurement mode.
    ///
    /// This constructor is side-effect free, so it will not write any
    /// configuration to the device until a first measurement is triggered.
    pub fn continuous(i2c: I2C, address: u8, delay: D) -> Self {
        MCP3425 {
            i2c,
            address,
            delay,
            mode: ContinuousMode,
            config: None,
        }
    }

    /// Change the conversion mode to one-shot.
    ///
    /// This conversion is side-effect free, so it will not write any
    /// configuration to the device until a first one-shot measurement is
    /// triggered.
    pub fn into_oneshot(self) -> MCP3425<I2C, D, OneShotMode> {
        MCP3425::oneshot(self.i2c, self.address, self.delay)
    }

    /// Write the specified configuration to the device and block until the
    /// first measurement is ready.
    ///
    /// The wait-for-measurement logic is implemented with polling, since there
    /// are no non-blocking `embedded_hal` traits yet.
    ///
    /// Note: Since the wait-until-ready logic needs to read the data register,
    /// when reading the measurement immediately after setting the
    /// configuration, that measurement will be returned as `NotFresh`.
    pub fn set_config(&mut self, config: &Config) -> Result<(), Error<I2C::Error>> {
        // Set configuration
        let command = self.mode.bits() | config.bits();
        self.i2c
            .write(self.address, &[command])
            .map(|()| self.config = Some(*config))
            .map_err(Error::I2c)?;

        // Determine time to wait for first measurement.
        // Values found by experimentation, these do not seem to be specified
        // in the datasheet.
        let sleep_ms = match config.resolution {
            Resolution::Bits12Sps240 => 4,
            Resolution::Bits14Sps60 => 15,
            Resolution::Bits16Sps15 => 57,
        };
        self.delay.delay_ms(sleep_ms);

        // Poll until ready
        let mut buf = [0, 0, 0];
        loop {
            self.i2c.read(self.address, &mut buf).map_err(Error::I2c)?;
            if (buf[2] & ConfigRegister::NOT_READY.bits()) == ConfigRegister::NOT_READY.bits() {
                // Not yet ready, wait some more time
                self.delay.delay_ms(1);
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Read a measurement from the device.
    ///
    /// Note that the [`set_config`](struct.MCP3425.html#method.set_config)
    /// method MUST have been called before, otherwise
    /// [`Error::NotInitialized`](enum.Error.html#variant.NotInitialized) will
    /// be returned.
    ///
    /// If you poll faster than the sample rate,
    /// [`Error::NotReady`](enum.Error.html#variant.NotReady) will be returned.
    pub fn read_measurement(&mut self) -> Result<Voltage, Error<I2C::Error>> {
        // Make sure that the configuration has been written to the device
        let config = self.config.ok_or(Error::NotInitialized)?;

        // Read measurement and config register
        let (measurement, config_reg) = self.read_i16_and_config()?;

        // Calculate voltage from raw value
        let voltage = self.calculate_voltage(measurement, &config.resolution)?;

        // Check "Not Ready" flag. See datasheet section 5.1.1 for more details.
        if config_reg.is_ready() {
            // The "Not Ready" flag is not set. This means the latest
            // conversion result is ready.
            Ok(voltage)
        } else {
            // The "Not Ready" flag is set. This means the conversion
            // result is not updated since the last reading. A new
            // conversion is under processing and the RDY bit will be
            // cleared when the new conversion result is ready.
            Err(Error::NotReady)
        }
    }
}

#[cfg(test)]
mod tests {
    use embedded_hal_mock::eh1::{
        delay::NoopDelay,
        i2c::{Mock as I2cMock, Transaction},
    };
    use rstest::rstest;

    use super::*;

    #[test]
    #[cfg(not(feature = "measurements"))]
    fn test_voltage_wrapper() {
        let a = Voltage::from_millivolts(2500);
        assert_eq!(a.as_millivolts(), 2500i16);
        assert_eq!(a.as_volts(), 2.5f32);

        let b = Voltage::from_millivolts(-100);
        assert_eq!(b.as_millivolts(), -100i16);
        assert_eq!(b.as_volts(), -0.1f32);
    }

    /// Instantiation in one-shot mode should not do any calls to the I2C bus.
    #[test]
    fn test_instantiation_oneshot() {
        let expectations = [];
        let dev = I2cMock::new(&expectations);
        let adc = MCP3425::oneshot(dev, 0x42, NoopDelay);
        adc.destroy().done();
    }

    /// Instantiation in continuous mode should not do any calls to the I2C bus.
    #[test]
    fn test_instantiation_continuous() {
        let expectations = [];
        let dev = I2cMock::new(&expectations);

        let adc = MCP3425::continuous(dev, 0x42, NoopDelay);
        adc.destroy().done();
    }

    /// Successfully measuring a voltage with default configuration.
    #[rstest]
    #[case(0b00000111, 0b11111110, 2046)] // Maximum (at 12 bits) - 1
    #[case(0b00000000, 0b00000111, 7)]
    #[case(0b00000000, 0b00000000, 0)]
    #[case(0b11111111, 0b11111111, -1)]
    #[case(0b11111000, 0b00000001, -2047)] // Minimum (at 12 bits) + 1
    #[cfg(not(feature = "measurements"))]
    fn test_read_voltage_oneshot(
        #[case] byte0: u8,
        #[case] byte1: u8,
        #[case] expected_millivolts: i16,
    ) {
        let addr = 0x42;
        let expectations = [
            // Write default config to config register:
            // - Bit 7: Initiate new conversion
            // - Bits 6-5: Channel selection (first channel)
            // - Bit 4: One-shot conversion mode
            // - Bits 3-2: Set sample rate 240 SPS
            // - Bits 1-0: Set PGA gain to 1
            Transaction::write(addr, vec![0b10000000]),
            // Device returns data
            Transaction::read(addr, vec![byte0, byte1, 0b00000000]),
        ];
        let dev = I2cMock::new(&expectations);
        let mut adc = MCP3425::oneshot(dev, addr, NoopDelay);
        let voltage = adc.measure(&Config::default()).expect("Measuring failed");
        assert_eq!(voltage.as_millivolts(), expected_millivolts);
        adc.destroy().done();
    }

    /// Test saturation at various resolutions.
    #[rstest]
    #[case(Resolution::Bits12Sps240, 0b10000000, 0b11111000, 0b00000111)] // 12 bits
    #[case(Resolution::Bits14Sps60, 0b10000100, 0b11100000, 0b00011111)] // 14 bits
    #[case(Resolution::Bits16Sps15, 0b10001000, 0b10000000, 0b01111111)] // 16 bits
    fn test_saturation(
        #[case] resolution: Resolution,
        #[case] config: u8,
        #[case] upper_byte_negative: u8,
        #[case] upper_byte_positive: u8,
    ) {
        let addr = 0x42;
        let expectations = [
            // Write config
            Transaction::write(addr, vec![config]),
            // Device returns data: Negative saturation
            Transaction::read(addr, vec![upper_byte_negative, 0b00000000, 0b00000000]),
            // Write config
            Transaction::write(addr, vec![config]),
            // Device returns data: Positive saturation
            Transaction::read(addr, vec![upper_byte_positive, 0b11111111, 0b00000000]),
        ];
        let dev = I2cMock::new(&expectations);
        let mut adc = MCP3425::oneshot(dev, addr, NoopDelay);

        // Test negative saturation
        let err_negative = adc
            .measure(&Config::default().with_resolution(resolution))
            .unwrap_err();
        assert!(
            matches!(err_negative, Error::VoltageTooLow),
            "{:?}",
            err_negative
        );

        // Test positive saturation
        let err_positive = adc
            .measure(&Config::default().with_resolution(resolution))
            .unwrap_err();
        assert!(
            matches!(err_positive, Error::VoltageTooHigh),
            "{:?}",
            err_positive
        );

        adc.destroy().done();
    }

    /// Test the "not ready" response handling.
    #[rstest]
    fn test_not_ready() {
        let addr = 0x42;
        let default_config = 0b10000000;
        let expectations = [
            // Write config
            Transaction::write(addr, vec![default_config]),
            // First bit in returned config register is set to 1 (not ready)
            Transaction::read(addr, vec![0b00000000, 0b00000000, 0b10000000]),
        ];
        let dev = I2cMock::new(&expectations);
        let mut adc = MCP3425::oneshot(dev, addr, NoopDelay);

        let err = adc.measure(&Config::default()).unwrap_err();
        assert!(matches!(err, Error::NotReady), "{:?}", err);

        adc.destroy().done();
    }

    /// Test that the configs are written correctly.
    #[rstest]
    #[case(Resolution::Bits14Sps60, Gain::Gain8, 0b10000111)]
    #[cfg(not(feature = "measurements"))]
    fn test_config(#[case] resolution: Resolution, #[case] gain: Gain, #[case] expected: u8) {
        let addr = 0x42;
        let expectations = [
            // Write config
            Transaction::write(addr, vec![expected]),
            Transaction::read(addr, vec![0b00000000, 0b00000000, 0b00000000]),
        ];
        let dev = I2cMock::new(&expectations);
        let mut adc = MCP3425::oneshot(dev, addr, NoopDelay);
        let voltage = adc
            .measure(
                &Config::default()
                    .with_resolution(resolution)
                    .with_gain(gain),
            )
            .expect("Measuring failed");
        assert_eq!(voltage.as_volts(), 0.0);
        assert_eq!(voltage.as_millivolts(), 0);
        adc.destroy().done();
    }
}
