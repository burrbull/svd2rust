use core::marker::PhantomData;
use embedded_hal::blocking::i2c::{WriteRead, WriteIter};

///This trait shows that register has `read` method
///
///Registers marked with `Writable` can be also `modify`'ed
pub trait Readable {}

///This trait shows that register has `write`, `write_with_zero` and `reset` method
///
///Registers marked with `Readable` can be also `modify`'ed
pub trait Writable {}

///Register size
pub trait Size {
    type Type;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum I2cError {
    Read(u16),
    Write(u16),
}

///Reset value of the register
///
///This value is initial value for `write` method.
///It can be also directly writed to register by `reset` method.
pub trait ResetValue: Size {
    ///Reset value of the register
    fn reset_value() -> Self::Type;
}

pub trait I2cAddress {
    const ADDRESS: u16;
}

pub struct I2cDevice<I2C>
where
    I2C: WriteRead + WriteIter,
{
    id: u8,
    _i2c: PhantomData<I2C>
}

impl<I2C> I2cDevice<I2C>
where
    I2C: WriteRead + WriteIter,
{
    #[inline(always)]
    pub fn new(id: u8) -> Self {
        Self { id, _i2c: PhantomData }
    }
    #[inline(always)]
    pub fn get_i2c_address(&self) -> u8 {
        self.id
    }
    #[inline(always)]
    pub fn set_i2c_address(&mut self, id: u8) {
        self.id = id
    }

    ///Reads the contents of `Readable` register
    #[inline(always)]
    pub fn read<U, REG>(&mut self, comm: &mut I2C, _reg: REG) -> Result<R<U, REG>, I2cError>
    where
        REG: Readable + I2cAddress + Size<Type = U>,
        U: Copy + FromU8Array,
        U::U8Array: Default + AsMut<[u8]>,
    {
        read_register(self.id, comm, REG::ADDRESS).map(|bits| R {
            bits,
            _reg: PhantomData,
        })
    }
    #[inline(always)]
    pub fn read_bits<U>(&mut self, comm: &mut I2C, addr: u16) -> Result<U, I2cError>
    where
        U: Copy + FromU8Array,
        U::U8Array: Default + AsMut<[u8]>,
    {
        read_register(self.id, comm, addr)
    }

    ///Writes bits to `Writable` register
    #[inline(always)]
    pub fn write<U, REG, F>(&self, comm: &mut I2C, _reg: REG, f: F) -> Result<(), I2cError>
    where
        F: FnOnce(&mut W<U, REG>) -> &mut W<U, REG>,
        REG: Writable + ResetValue + I2cAddress + Size<Type = U>,
        U: Copy + IntoU8Array,
        U::U8Array: AsRef<[u8]>,
    {
        write_register(
            self.id,
            comm,
            REG::ADDRESS,
            f(&mut W {
                bits: REG::reset_value(),
                _reg: PhantomData,
            })
            .bits,
        )
    }
    ///Writes the reset value to `Writable` register
    #[inline(always)]
    pub fn reset<U, REG>(&self, comm: &mut I2C, _reg: REG) -> Result<(), I2cError>
    where
        REG: Writable + ResetValue + I2cAddress + Size<Type = U>,
        U: Copy + IntoU8Array,
        U::U8Array: AsRef<[u8]>,
    {
        write_register(self.id, comm, REG::ADDRESS, REG::reset_value())
    }
    ///Writes bits to `Writable` register
    #[inline(always)]
    pub fn write_with_zero<U, REG, F>(
        &self,
        comm: &mut I2C,
        _reg: REG,
        f: F,
    ) -> Result<(), I2cError>
    where
        F: FnOnce(&mut W<U, REG>) -> &mut W<U, REG>,
        REG: Writable + ResetValue + I2cAddress + Size<Type = U>,
        U: Copy + Default + IntoU8Array,
        U::U8Array: AsRef<[u8]>,
    {
        write_register(
            self.id,
            comm,
            REG::ADDRESS,
            f(&mut W {
                bits: U::default(),
                _reg: PhantomData,
            })
            .bits,
        )
    }

    ///Writes bits to `Writable` register
    #[inline(always)]
    pub unsafe fn write_bits<U>(
        &self,
        comm: &mut I2C,
        addr: u16,
        bits: U,
    ) -> Result<(), I2cError>
    where
        U: Copy + Default + IntoU8Array,
        U::U8Array: AsRef<[u8]>,
    {
        write_register(self.id, comm, addr, bits)
    }

    ///Modifies the contents of the register
    #[inline(always)]
    pub fn modify<U, REG, F>(&self, comm: &mut I2C, _reg: REG, f: F) -> Result<(), I2cError>
    where
        for<'w> F: FnOnce(&R<U, REG>, &'w mut W<U, REG>) -> &'w mut W<U, REG>,
        REG: Readable + Writable + I2cAddress + Size<Type = U>,
        U: Copy + FromU8Array + IntoU8Array,
        U::U8Array: Default + AsRef<[u8]> + AsMut<[u8]>,
    {
        let bits = read_register(self.id, comm, REG::ADDRESS)?;
        write_register(
            self.id,
            comm,
            REG::ADDRESS,
            f(
                &R {
                    bits,
                    _reg: PhantomData,
                },
                &mut W {
                    bits,
                    _reg: PhantomData,
                },
            )
            .bits,
        )
    }
}

fn read_register<U, Comm>(id: u8, comm: &mut Comm, addr: u16) -> Result<U, I2cError>
where
    U: crate::FromU8Array,
    U::U8Array: Default + AsMut<[u8]>,
    Comm: WriteRead,
{
    let mut data: U::U8Array = Default::default();
    comm.write_read(id, &addr.into_u8_array(), data.as_mut())
        .map_err(|_| I2cError::Read(addr))?;
    Ok(U::from_u8_array(data))
}

#[inline(always)]
fn write_register<Comm, U>(id: u8, comm: &mut Comm, addr: u16, bits: U) -> Result<(), I2cError>
where
    Comm: WriteIter,
    U: IntoU8Array,
    U::U8Array: AsRef<[u8]>,
{
    comm.write(
        id,
        addr.into_u8_array()
            .iter()
            .chain(bits.into_u8_array().as_ref().iter())
            .cloned(),
    )
    .map_err(|_| I2cError::Write(addr))
}

pub trait AssociatedU8Array {
    type U8Array;
}

pub trait IntoU8Array: AssociatedU8Array {
    fn into_u8_array(self) -> Self::U8Array;
}

pub trait FromU8Array: AssociatedU8Array {
    fn from_u8_array(a: Self::U8Array) -> Self;
}

impl AssociatedU8Array for u8 {
    type U8Array = [u8; 1];
}

impl AssociatedU8Array for u16 {
    type U8Array = [u8; 2];
}

impl AssociatedU8Array for u32 {
    type U8Array = [u8; 4];
}

impl IntoU8Array for u8 {
    fn into_u8_array(self) -> Self::U8Array {
        self.to_le_bytes()
    }
}

impl FromU8Array for u8 {
    fn from_u8_array(a: Self::U8Array) -> Self {
        Self::from_le_bytes(a)
    }
}

impl IntoU8Array for u16 {
    fn into_u8_array(self) -> Self::U8Array {
        self.to_le_bytes()
    }
}

impl FromU8Array for u16 {
    fn from_u8_array(a: Self::U8Array) -> Self {
        Self::from_le_bytes(a)
    }
}

impl IntoU8Array for u32 {
    fn into_u8_array(self) -> Self::U8Array {
        self.to_le_bytes()
    }
}

impl FromU8Array for u32 {
    fn from_u8_array(a: Self::U8Array) -> Self {
        Self::from_le_bytes(a)
    }
}

///Register/field reader
///
///Result of the [`read`](I2cDevice::read) method of device.
pub struct R<U, T> {
    pub(crate) bits: U,
    _reg: PhantomData<T>,
}

impl<U, T> R<U, T>
where
    U: Copy,
{
    ///Create new instance of reader
    #[inline(always)]
    pub(crate) fn new(bits: U) -> Self {
        Self {
            bits,
            _reg: PhantomData,
        }
    }
    ///Read raw bits from register/field
    #[inline(always)]
    pub fn bits(&self) -> U {
        self.bits
    }
}

impl<U, T, FI> PartialEq<FI> for R<U, T>
where
    U: PartialEq + From<FI>,
    FI: Copy,
{
    #[inline(always)]
    fn eq(&self, other: &FI) -> bool {
        self.bits.eq(&(*other).into())
    }
}

impl<FI> R<bool, FI> {
    ///Value of the field as raw bits
    #[inline(always)]
    pub fn bit(&self) -> bool {
        self.bits
    }
    ///Returns `true` if the bit is clear (0)
    #[inline(always)]
    pub fn bit_is_clear(&self) -> bool {
        !self.bit()
    }
    ///Returns `true` if the bit is set (1)
    #[inline(always)]
    pub fn bit_is_set(&self) -> bool {
        self.bit()
    }
}

///Register writer
///
///Used as an argument to the closures in the [`write`](I2cDevice::write) method of device
pub struct W<U, REG> {
    ///Writable bits
    pub(crate) bits: U,
    _reg: PhantomData<REG>,
}

impl<U, REG> W<U, REG> {
    ///Writes raw bits to the register
    #[inline(always)]
    pub unsafe fn bits(&mut self, bits: U) -> &mut Self {
        self.bits = bits;
        self
    }
}

///Used if enumerated values cover not the whole range
#[derive(Clone, Copy, PartialEq)]
pub enum Variant<U, T> {
    ///Expected variant
    Val(T),
    ///Raw bits
    Res(U),
}

use Variant::*;
impl<U, T> Variant<U, T> {
    /// Check if the variant is expected
    pub fn is_value(&self) -> bool {
        match self {
            Val(_) => true,
            Res(_) => false,
        }
    }

    /// Check if the variant is not expected
    pub fn is_reserved(&self) -> bool {
        match self {
            Val(_) => false,
            Res(_) => true,
        }
    }

    /// Moves the value `v` out of the `Variant` if it is `Val(v)`.
    ///
    /// Panics if the self value equals `Res`
    #[inline]
    pub fn unwrap(self) -> T {
        match self {
            Val(v) => v,
            Res(_) => panic!("Unexpected variant"),
        }
    }

    /// Returns the contained value or a default
    #[inline]
    pub fn unwrap_or(self, def: T) -> T {
        match self {
            Val(v) => v,
            Res(_) => def,
        }
    }

    /// Returns the contained value or computes it from a closure
    #[inline]
    pub fn unwrap_or_else<F: FnOnce(U) -> T>(self, f: F) -> T {
        match self {
            Val(v) => v,
            Res(u) => f(u),
        }
    }

    /// Unwraps a result, yielding the content of an `Val`.
    ///
    /// Panics if the value is an `Res`, with a panic message including the
    /// passed message, and the content of the `Res`.
    pub fn expect(self, msg: &'static str) -> T {
        match self {
            Val(v) => v,
            Res(_) => panic!(msg),
        }
    }
}
