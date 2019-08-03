use core::marker;

///This trait shows that register has `read` method
///
///Registers marked with `Writable` can be also `modify`'ed
pub trait Readable {}

///This trait shows that register has `write`, `write_with_zero` and `reset` method
///
///Registers marked with `Readable` can be also `modify`'ed
pub trait Writable {}

///Shows memory size of register/field
pub trait SizeType {
    ///Register or field unsigned size type
    type Type;
}

impl SizeType for bool {
    type Type = bool;
}

impl SizeType for u8 {
    type Type = u8;
}

impl SizeType for u16 {
    type Type = u16;
}

impl SizeType for u32 {
    type Type = u32;
}

impl SizeType for u64 {
    type Type = u64;
}

///Reset value of the register
///
///This value is initial value for `write` method.
///It can be also directly writed to register by `reset` method.
pub trait ResetValue: SizeType {
    ///Reset value of the register
    const RESET_VALUE: Self::Type;
}

///Converting enumerated values to bits
pub trait ToBits: SizeType {
    ///Conversion method
    fn to_bits(&self) -> Self::Type;
}

///This structure provides volatile access to register
pub struct Reg<U, REG> {
    register: vcell::VolatileCell<U>,
    _marker: marker::PhantomData<REG>,
}

unsafe impl<U: Send, REG> Send for Reg<U, REG> { }

impl<U, REG> crate::SizeType for Reg<U, REG> {
    type Type = U;
}

impl<U, REG> Reg<U, REG>
where
    Self: Readable,
    U: Copy
{
    ///Reads the contents of `Readable` register
    ///
    ///See [reading](https://rust-embedded.github.io/book/start/registers.html#reading) in book.
    #[inline(always)]
    pub fn read(&self) -> R<Self> {
        R {bits: self.register.get()}
    }
}

impl<U, REG> Reg<U, REG>
where
    Self: ResetValue<Type=U> + Writable,
    U: Copy,
{
    ///Writes the reset value to `Writable` register
    #[inline(always)]
    pub fn reset(&self) {
        self.register.set(Self::RESET_VALUE)
    }
}

impl<U, REG> Reg<U, REG>
where
    Self: ResetValue<Type=U> + Writable,
    U: Copy
{
    ///Writes bits to `Writable` register
    ///
    ///See [writing](https://rust-embedded.github.io/book/start/registers.html#writing) in book.
    #[inline(always)]
    pub fn write<F>(&self, f: F)
    where
        F: FnOnce(&mut W<Self>) -> &mut W<Self>
    {
        self.register.set(f(&mut W {bits: Self::RESET_VALUE}).bits);
    }
}

impl<U, REG> Reg<U, REG>
where
    Self: Writable,
    U: Copy + Default
{
    ///Writes Zero to `Writable` register
    #[inline(always)]
    pub fn write_with_zero<F>(&self, f: F)
    where
        F: FnOnce(&mut W<Self>) -> &mut W<Self>
    {
        self.register.set(f(&mut W {bits: U::default()}).bits);
    }
}

impl<U, REG> Reg<U, REG>
where
    Self: Readable + Writable,
    U: Copy,
{
    ///Modifies the contents of the register
    ///
    ///See [modifying](https://rust-embedded.github.io/book/start/registers.html#modifying) in book.
    #[inline(always)]
    pub fn modify<F>(&self, f: F)
    where
        for<'w> F: FnOnce(&R<Self>, &'w mut W<Self>) -> &'w mut W<Self>
    {
        let bits = self.register.get();
        self.register.set(f(&R {bits}, &mut W {bits}).bits);
    }
}

///Register/field reader
pub struct R<T> where T: SizeType {
    pub(crate) bits: T::Type,
}

impl<T> R<T>
where
    T: SizeType,
    T::Type: Copy
{
    ///Read raw bits from register/field
    #[inline(always)]
    pub fn bits(&self) -> T::Type {
        self.bits
    }
}

impl<FI> PartialEq<FI> for R<FI>
where
    FI: SizeType + ToBits,
    FI::Type: PartialEq,
{
    #[inline(always)]
    fn eq(&self, other: &FI) -> bool {
        self.bits.eq(&other.to_bits())
    }
}

///Bit access methods for 1-bit wise field
impl<FI> R<FI>
where
    FI: SizeType<Type=bool>
{
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
pub struct W<REG> where REG: SizeType {
    ///Writable bits
    pub bits: REG::Type,
}

impl<REG> W<REG>
where
    REG: SizeType,
{
    ///Writes raw bits to the register
    #[inline(always)]
    pub unsafe fn bits(&mut self, bits: REG::Type) -> &mut Self {
        self.bits = bits;
        self
    }
}

///Used if enumerated values cover not the whole range
#[derive(Clone,Copy,PartialEq)]
pub enum Variant<FI>
where
    FI: SizeType
{
    ///Expected variant
    Val(FI),
    ///Raw bits
    Res(FI::Type),
}
