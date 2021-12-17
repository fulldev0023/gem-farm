use std::convert::TryFrom;

use anchor_lang::prelude::*;

use crate::errors::ErrorCode;

// --------------------------------------- traits

pub trait TrySub: Sized {
    fn try_sub(self, rhs: Self) -> Result<Self, ProgramError>;
    fn try_sub_assign(&mut self, rhs: Self) -> ProgramResult;
}

pub trait TryAdd: Sized {
    fn try_add(self, rhs: Self) -> Result<Self, ProgramError>;
    fn try_add_assign(&mut self, rhs: Self) -> ProgramResult;
}

pub trait TryDiv: Sized {
    fn try_floor_div(self, rhs: Self) -> Result<Self, ProgramError>;
    fn try_floor_div_assign(&mut self, rhs: Self) -> ProgramResult;
    fn try_ceil_div(self, rhs: Self) -> Result<Self, ProgramError>;
    fn try_ceil_div_assign(&mut self, rhs: Self) -> ProgramResult;
    fn try_rounded_div(self, rhs: Self) -> Result<Self, ProgramError>;
    fn try_rounded_div_assign(&mut self, rhs: Self) -> ProgramResult;
}

pub trait TryMul: Sized {
    fn try_mul(self, rhs: Self) -> Result<Self, ProgramError>;
    fn try_mul_assign(&mut self, rhs: Self) -> ProgramResult;
}

pub trait TryPow: Sized {
    fn try_pow(self, rhs: u32) -> Result<Self, ProgramError>;
    fn try_pow_assign(&mut self, rhs: u32) -> ProgramResult;
}

// pub trait TrySqrt: Sized {
//     fn try_sqrt(self) -> Result<Self, ProgramError>;
//     fn try_sqrt_assign(&mut self, rhs: Self) -> ProgramResult;
// }

pub trait TryRem: Sized {
    fn try_rem(self, rhs: Self) -> Result<Self, ProgramError>;
}

// pub trait TryCast<Into>: Sized {
//     fn try_cast(self) -> Result<Into, ProgramError>;
// }

// --------------------------------------- impl

macro_rules! try_math {
    ($our_type:ty) => {
        impl TrySub for $our_type {
            fn try_sub(self, rhs: Self) -> Result<Self, ProgramError> {
                self.checked_sub(rhs)
                    .ok_or(ErrorCode::ArithmeticError.into())
            }
            fn try_sub_assign(&mut self, rhs: Self) -> ProgramResult {
                *self = self.try_sub(rhs)?;
                Ok(())
            }
        }

        impl TryAdd for $our_type {
            fn try_add(self, rhs: Self) -> Result<Self, ProgramError> {
                self.checked_add(rhs)
                    .ok_or(ErrorCode::ArithmeticError.into())
            }
            fn try_add_assign(&mut self, rhs: Self) -> ProgramResult {
                *self = self.try_add(rhs)?;
                Ok(())
            }
        }

        impl TryDiv for $our_type {
            fn try_floor_div(self, rhs: Self) -> Result<Self, ProgramError> {
                self.checked_div(rhs)
                    .ok_or(ErrorCode::ArithmeticError.into())
            }
            fn try_floor_div_assign(&mut self, rhs: Self) -> ProgramResult {
                *self = self.try_floor_div(rhs)?;
                Ok(())
            }
            fn try_ceil_div(self, rhs: Self) -> Result<Self, ProgramError> {
                let reduced_by_one = self
                    .checked_sub(1)
                    .ok_or::<ProgramError>(ErrorCode::ArithmeticError.into())?;

                let divided = reduced_by_one
                    .checked_div(rhs)
                    .ok_or::<ProgramError>(ErrorCode::ArithmeticError.into())?;

                (1 as $our_type)
                    .checked_add(divided)
                    .ok_or(ErrorCode::ArithmeticError.into())
            }
            fn try_ceil_div_assign(&mut self, rhs: Self) -> ProgramResult {
                *self = self.try_ceil_div(rhs)?;
                Ok(())
            }
            fn try_rounded_div(self, rhs: Self) -> Result<Self, ProgramError> {
                let rounding = rhs
                    .checked_div(2)
                    .ok_or::<ProgramError>(ErrorCode::ArithmeticError.into())?;

                let with_rounding = self
                    .checked_add(rounding)
                    .ok_or::<ProgramError>(ErrorCode::ArithmeticError.into())?;

                with_rounding
                    .checked_div(rhs)
                    .ok_or(ErrorCode::ArithmeticError.into())
            }
            fn try_rounded_div_assign(&mut self, rhs: Self) -> ProgramResult {
                *self = self.try_rounded_div(rhs)?;
                Ok(())
            }
        }

        impl TryMul for $our_type {
            fn try_mul(self, rhs: Self) -> Result<Self, ProgramError> {
                self.checked_mul(rhs)
                    .ok_or(ErrorCode::ArithmeticError.into())
            }
            fn try_mul_assign(&mut self, rhs: Self) -> ProgramResult {
                *self = self.try_mul(rhs)?;
                Ok(())
            }
        }

        impl TryPow for $our_type {
            fn try_pow(self, rhs: u32) -> Result<Self, ProgramError> {
                self.checked_pow(rhs)
                    .ok_or(ErrorCode::ArithmeticError.into())
            }
            fn try_pow_assign(&mut self, rhs: u32) -> ProgramResult {
                *self = self.try_pow(rhs)?;
                Ok(())
            }
        }

        impl TryRem for $our_type {
            fn try_rem(self, rhs: Self) -> Result<Self, ProgramError> {
                self.checked_rem(rhs)
                    .ok_or(ErrorCode::ArithmeticError.into())
            }
        }
    };
}

pub(crate) use try_math;

try_math! {u8}
try_math! {i8}
try_math! {u16}
try_math! {i16}
try_math! {u32}
try_math! {i32}
try_math! {u64}
try_math! {i64}
try_math! {u128}
try_math! {i128}

#[cfg(test)]
mod tests {
    use super::*;

    // --------------------------------------- dividison types

    #[test]
    fn test_floor_div() {
        //the easy (no remainder) case
        let x = 10_u64;
        let y = 5;
        let r = x.try_floor_div(y).unwrap();
        assert_eq!(r, 2);

        //<.5 case (2.2)
        let x = 11_u64;
        let y = 5;
        let r = x.try_floor_div(y).unwrap();
        assert_eq!(r, 2);

        //>.5 case (2.8)
        let x = 14_u64;
        let y = 5;
        let r = x.try_floor_div(y).unwrap();
        assert_eq!(r, 2);

        //.5 case
        let x = 5_u64;
        let y = 2;
        let r = x.try_floor_div(y).unwrap();
        assert_eq!(r, 2);
    }

    #[test]
    fn test_ceil_div() {
        //the easy (no remainder) case
        let x = 10_u64;
        let y = 5;
        let r = x.try_ceil_div(y).unwrap();
        assert_eq!(r, 2);

        //<.5 case (2.2)
        let x = 11_u64;
        let y = 5;
        let r = x.try_ceil_div(y).unwrap();
        assert_eq!(r, 3);

        //>.5 case (2.8)
        let x = 14_u64;
        let y = 5;
        let r = x.try_ceil_div(y).unwrap();
        assert_eq!(r, 3);

        //.5 case
        let x = 5_u64;
        let y = 2;
        let r = x.try_ceil_div(y).unwrap();
        assert_eq!(r, 3);
    }

    #[test]
    fn test_rounded_div() {
        //the easy (no remainder) case
        let x = 10_u64;
        let y = 5;
        let r = x.try_rounded_div(y).unwrap();
        assert_eq!(r, 2);

        //<.5 case (2.2)
        let x = 11_u64;
        let y = 5;
        let r = x.try_rounded_div(y).unwrap();
        assert_eq!(r, 2);

        //>.5 case (2.8)
        let x = 14_u64;
        let y = 5;
        let r = x.try_rounded_div(y).unwrap();
        assert_eq!(r, 3);

        //.5 case
        let x = 5_u64;
        let y = 2;
        let r = x.try_rounded_div(y).unwrap();
        assert_eq!(r, 3);
    }

    // --------------------------------------- assigns

    #[test]
    fn test_add_assign() {
        let mut x = 10_u64;
        let y = 2;
        x.try_add_assign(y).unwrap();
        assert_eq!(x, 12);
    }

    #[test]
    fn test_sub_assign() {
        let mut x = 10_u64;
        let y = 2;
        x.try_sub_assign(y).unwrap();
        assert_eq!(x, 8);
    }

    #[test]
    fn test_floor_div_assign() {
        let mut x = 10_u64;
        let y = 3;
        x.try_floor_div_assign(y).unwrap();
        assert_eq!(x, 3);
    }

    #[test]
    fn test_ceil_div_assign() {
        let mut x = 10_u64;
        let y = 3;
        x.try_ceil_div_assign(y).unwrap();
        assert_eq!(x, 4);
    }

    #[test]
    fn test_rounded_div_assign() {
        let mut x = 10_u64;
        let y = 3;
        x.try_rounded_div_assign(y).unwrap();
        assert_eq!(x, 3);
    }

    #[test]
    fn test_mul_assign() {
        let mut x = 10_u64;
        let y = 2;
        x.try_mul_assign(y).unwrap();
        assert_eq!(x, 20);
    }

    #[test]
    fn test_pow_assign() {
        let mut x = 10_u64;
        let y = 2;
        x.try_pow_assign(y).unwrap();
        assert_eq!(x, 100);
    }
}
