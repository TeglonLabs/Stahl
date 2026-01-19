use crate::rvals::{IntoStahlVal, Result, StahlComplex, StahlVal};
use crate::{stahlerr, stop};
use num::Zero;
use num::{
    pow::Pow, BigInt, BigRational, CheckedAdd, CheckedMul, Integer, Rational32, Signed, ToPrimitive,
};
use std::ops::Neg;

/// Checks if the given value is a number
///
/// (number? value) -> boolean?
///
/// * value : any - The value to check
///
/// # Examples
/// ```scheme
/// > (number? 42) ;; => #t
/// > (number? "hello") ;; => #f
/// > (number? 'symbol) ;; => #f
/// ```
#[stahl_derive::function(name = "number?", constant = true)]
pub fn numberp(value: &StahlVal) -> bool {
    matches!(
        value,
        StahlVal::IntV(_)
            | StahlVal::BigNum(_)
            | StahlVal::Rational(_)
            | StahlVal::BigRational(_)
            | StahlVal::NumV(_)
            | StahlVal::Complex(_)
    )
}

/// Checks if the given value is a complex number
///
/// (complex? value) -> boolean?
///
/// * value : any - The value to check
///
/// # Examples
/// ```scheme
/// > (complex? 3+4i) ;; => #t
/// > (complex? 42) ;; => #t
/// > (complex? "hello") ;; => #f
/// ```
#[stahl_derive::function(name = "complex?", constant = true)]
pub fn complexp(value: &StahlVal) -> bool {
    numberp(value)
}

/// Checks if the given value is a real number
///
/// (real? value) -> boolean?
///
/// * value : any - The value to check
///
/// # Examples
/// ```scheme
/// > (real? 42) ;; => #t
/// > (real? 3+4i) ;; => #f
/// > (real? "hello") ;; => #f
/// ```
#[stahl_derive::function(name = "real?", constant = true)]
pub fn realp(value: &StahlVal) -> bool {
    matches!(
        value,
        StahlVal::IntV(_)
            | StahlVal::BigNum(_)
            | StahlVal::Rational(_)
            | StahlVal::BigRational(_)
            | StahlVal::NumV(_)
    )
}

/// Returns #t if obj is a rational number, #f otherwise.
/// Rational numbers are numbers that can be expressed as the quotient of two numbers.
/// For example, 3/4, -5/2, 0.25, and 0 are rational numbers.
///
/// (rational? value) -> bool?
///
/// * value : any - The value to check
///
/// Examples:
/// ```scheme
/// > (rational? (/ 0.0)) ;; => #f
/// > (rational? 3.5) ;; => #t
/// > (rational? 6/10) ;; => #t
/// > (rational? +nan.0) ;; => #f
/// ```
#[stahl_derive::function(name = "rational?", constant = true)]
fn rationalp(value: &StahlVal) -> bool {
    match value {
        StahlVal::IntV(_)
        | StahlVal::BigNum(_)
        | StahlVal::Rational(_)
        | StahlVal::BigRational(_) => true,
        StahlVal::NumV(n) => n.is_finite(),
        _ => false,
    }
}

/// Checks if the given value is an integer, an alias for `integer?`
///
/// (int? value) -> boolean?
///
/// * value : any - The value to check
///
/// # Examples
/// ```scheme
/// > (int? 42) ;; => #t
/// > (int? 3.14) ;; => #f
/// > (int? "hello") ;; => #f
/// ```
#[stahl_derive::function(name = "int?", constant = true)]
fn intp(value: &StahlVal) -> bool {
    match value {
        StahlVal::IntV(_) | StahlVal::BigNum(_) => true,
        StahlVal::NumV(n) if n.fract() == 0.0 => true,
        _ => false,
    }
}

/// Checks if the given value is an integer, an alias for `int?`
///
/// (integer? value) -> boolean?
///
/// * value : any - The value to check
///
/// # Examples
/// ```scheme
/// > (integer? 42) ;; => #t
/// > (integer? 3.14) ;; => #f
/// > (integer? "hello") ;; => #f
/// ```
#[stahl_derive::function(name = "integer?", constant = true)]
fn integerp(value: &StahlVal) -> bool {
    intp(value)
}

/// Checks if the given value is an exact integer
///
/// (exact-integer? value) -> boolean?
///
/// * value : any - The value to check
///
/// # Examples
/// ```scheme
/// > (exact-integer? 42) ;; => #t
/// > (exact-integer? -42) ;; => #t
/// > (exact-integer? 4.0) ;; => #f
/// ```
#[stahl_derive::function(name = "exact-integer?", constant = true)]
fn exact_integerp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::IntV(_) | StahlVal::BigNum(_))
}

/// Checks if the given value is a floating-point number
///
/// (float? value) -> boolean?
///
/// * value : any - The value to check
///
/// # Examples
/// ```scheme
/// > (float? 42) ;; => #f
/// > (float? 3.14) ;; => #t
/// > (float? #t) ;; => #f
/// ```
#[stahl_derive::function(name = "float?", constant = true)]
fn floatp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::NumV(_))
}

/// Returns `#t` if the real number is Nan.
///
/// (nan? value) -> boolean?
///
/// * value : real? - The value to check
///
/// ```scheme
/// (nan? +nan.0) => #t
/// (nan? 100000) => #f
/// ```
#[stahl_derive::function(name = "nan?", constant = true)]
fn nanp(value: &StahlVal) -> Result<StahlVal> {
    match value {
        StahlVal::NumV(n) => n.is_nan().into_stahlval(),
        // The following types are numbers but can not be nan.
        StahlVal::IntV(_)
        | StahlVal::Rational(_)
        | StahlVal::BigNum(_)
        | StahlVal::BigRational(_) => false.into_stahlval(),
        _ => stahlerr!(TypeMismatch => "nan? expected real number"),
    }
}

/// Checks if the given real number is zero.
///
/// (zero? num) -> boolean?
///
/// * num : real? - The number to check for zero.
///
/// # Examples
/// ```scheme
/// > (zero? 0) ;; => #t
/// > (zero? 0.0) ;; => #t
/// > (zero? 0.1) ;; => #f
/// ```
#[stahl_derive::function(name = "zero?", constant = true)]
fn zerop(value: &StahlVal) -> Result<StahlVal> {
    match value {
        StahlVal::NumV(x) => x.is_zero().into_stahlval(),
        StahlVal::IntV(0) => true.into_stahlval(),
        // The following types are numbers, but are casted to NumV or IntV if they are 0 by their
        // into_stahlval implementation.
        StahlVal::IntV(_)
        | StahlVal::Rational(_)
        | StahlVal::BigNum(_)
        | StahlVal::BigRational(_)
        | StahlVal::Complex(_) => false.into_stahlval(),
        _ => stahlerr!(TypeMismatch => "zero? expected number"),
    }
}

/// Checks if the given real number is positive.
///
/// (positive? num) -> boolean?
///
/// * num : real? - The real number to check for positivity.
///
/// # Examples
/// ```scheme
/// > (positive? 0) ;; => #f
/// > (positive? 1) ;; => #t
/// > (positive? -1) ;; => #f
/// ```
#[stahl_derive::function(name = "positive?", constant = true)]
fn positivep(value: &StahlVal) -> Result<StahlVal> {
    match value {
        StahlVal::NumV(n) => n.is_positive().into_stahlval(),
        StahlVal::IntV(n) => n.is_positive().into_stahlval(),
        StahlVal::Rational(n) => n.is_positive().into_stahlval(),
        StahlVal::BigNum(n) => n.is_positive().into_stahlval(),
        StahlVal::BigRational(n) => n.is_positive().into_stahlval(),
        _ => stahlerr!(TypeMismatch => "positive? expected real number"),
    }
}

/// Checks if the given real number is negative.
///
/// (negative? num) -> boolean?
///
/// * num : real? - The real number to check for negativity.
///
/// # Examples
/// ```scheme
/// > (negative? 0) ;; => #f
/// > (negative? 1) ;; => #f
/// > (negative? -1) ;; => #t
/// ```
#[stahl_derive::function(name = "negative?", constant = true)]
fn negativep(value: &StahlVal) -> Result<StahlVal> {
    match value {
        StahlVal::NumV(n) => n.is_negative().into_stahlval(),
        StahlVal::IntV(n) => n.is_negative().into_stahlval(),
        StahlVal::Rational(n) => n.is_negative().into_stahlval(),
        StahlVal::BigNum(n) => n.is_negative().into_stahlval(),
        StahlVal::BigRational(n) => n.is_negative().into_stahlval(),
        _ => stahlerr!(TypeMismatch => "negative? expected real number"),
    }
}

/// Subtracts the given numbers.
///
/// (- . nums) -> number?
///
/// * nums : number? - The numbers to subtract. Must have at least one number.
///
/// # Examples
/// ```scheme
/// > (- 5 3) ;; => 2
/// > (- 10 3 2) ;; => 5
/// > (- -5) ;; => 5
/// ```
#[stahl_derive::native(name = "-", constant = true, arity = "AtLeast(1)")]
pub fn subtract_primitive(args: &[StahlVal]) -> Result<StahlVal> {
    ensure_args_are_numbers("-", args)?;
    match args {
        [] => stahlerr!(TypeMismatch => "- requires at least one argument"),
        [x] => negate(x),
        [x, ys @ ..] => {
            let y = negate(&add_primitive_no_check(ys)?)?;
            add_two(x, &y)
        }
    }
}

#[inline(always)]
fn add_primitive_no_check(args: &[StahlVal]) -> Result<StahlVal> {
    match args {
        [] => 0.into_stahlval(),
        [x] => x.clone().into_stahlval(),
        [x, y] => add_two(x, y),
        [x, y, zs @ ..] => {
            let mut res = add_two(x, y)?;
            for z in zs {
                res = add_two(&res, z)?;
            }
            res.into_stahlval()
        }
    }
}

/// Adds the given numbers.
///
/// (+ . nums) -> number?
///
/// * nums : number? - The numbers to add. Can have any number of arguments including zero.
///
/// # Examples
/// ```scheme
/// > (+ 5 3) ;; => 8
/// > (+ 10 3 2) ;; => 15
/// > (+) ;; => 0
/// ```
#[stahl_derive::native(name = "+", constant = true, arity = "AtLeast(0)")]
pub fn add_primitive(args: &[StahlVal]) -> Result<StahlVal> {
    ensure_args_are_numbers("+", args)?;
    match args {
        [] => 0.into_stahlval(),
        [x] => x.clone().into_stahlval(),
        [x, y] => add_two(x, y),
        [x, y, zs @ ..] => {
            let mut res = add_two(x, y)?;
            for z in zs {
                res = add_two(&res, z)?;
            }
            res.into_stahlval()
        }
    }
}

/// Multiplies the given numbers.
///
/// (* . nums) -> number?
///
/// * nums : number? - The numbers to multiply. Can have any number of arguments including zero.
///
/// # Examples
/// ```scheme
/// > (* 5 3) ;; => 15
/// > (* 10 3 2) ;; => 60
/// > (*) ;; => 1
/// ```
#[stahl_derive::native(name = "*", constant = true, arity = "AtLeast(0)")]
pub fn multiply_primitive(args: &[StahlVal]) -> Result<StahlVal> {
    ensure_args_are_numbers("*", args)?;
    multiply_primitive_impl(args)
}

/// Returns quotient of dividing numerator by denomintator.
///
/// (quotient numerator denominator) -> integer?
///
/// * numerator : integer? - The numerator.
/// * denominator : integer? - The denominator.
///
/// # Examples
/// ```scheme
/// > (quotient 11 2) ;; => 5
/// > (quotient 10 2) ;; => 5
/// > (quotient -10 2) ;; => -5
/// ```
#[stahl_derive::native(name = "quotient", constant = true, arity = "Exact(2)")]
pub fn quotient(args: &[StahlVal]) -> Result<StahlVal> {
    match &args {
        [l, r] => match (l, r) {
            (StahlVal::IntV(l), StahlVal::IntV(r)) => (l / r).into_stahlval(),
            _ => stahlerr!(TypeMismatch => "quotient only supports integers"),
        },
        _ => stahlerr!(ArityMismatch => "quotient requires 2 arguments"),
    }
}

/// Returns the euclidean remainder of the division of the first number by the second
/// This differs from the remainder operator when using negative numbers.
///
/// (modulo n m) -> integer?
///
/// * n : integer?
/// * m : integer?
///
/// # Examples
/// ```scheme
/// > (modulo 10 3) ;; => 1
/// > (modulo -10 3) ;; => 2
/// > (modulo 10 -3) ;; => -2
/// > (module -10 -3) ;; => -1
/// ```
#[stahl_derive::native(name = "modulo", constant = true, arity = "Exact(2)")]
pub fn modulo(args: &[StahlVal]) -> Result<StahlVal> {
    match &args {
        [l, r] => match (l, r) {
            (StahlVal::IntV(l), StahlVal::IntV(r)) => ((l % r + r) % r).into_stahlval(),
            _ => stahlerr!(TypeMismatch => "modulo only supports integers"),
        },
        _ => stahlerr!(ArityMismatch => "modulo requires 2 arguments"),
    }
}

/// Returns the arithmetic remainder of the division of the first number by the second.
/// This differs from the modulo operator when using negative numbers.
///
/// (remainder n m) -> integer?
///
/// * n : integer?
/// * m : integer?
///
/// # Examples
/// ```scheme
/// > (remainder 10 3) ;; => 1
/// > (remainder -10 3) ;; => -1
/// > (remainder 10 -3) ;; => 1
/// > (remainder -10 -3) ;; => -1
/// ```
#[stahl_derive::native(name = "remainder", constant = true, arity = "Exact(2)")]
pub fn remainder(args: &[StahlVal]) -> Result<StahlVal> {
    match &args {
        [l, r] => match (l, r) {
            (StahlVal::IntV(l), StahlVal::IntV(r)) => (l % r).into_stahlval(),
            _ => stahlerr!(TypeMismatch => "remainder only supports integers"),
        },
        _ => stahlerr!(ArityMismatch => "remainder requires 2 arguments"),
    }
}

/// Returns the sine value of the input angle, measured in radians.
///
/// (sin n) -> number?
///
/// * n : number? - The input angle, in radians.
///
/// # Examples
/// ```scheme
/// > (sin 0) ;; => 0
/// > (sin 1) ;; => 0.8414709848078965
/// > (sin 2.0) ;; => 0.9092974268256817
/// > (sin 3.14) ;; => 0.0015926529164868282
/// ```
#[stahl_derive::function(name = "sin", constant = true)]
pub fn sin(arg: &StahlVal) -> Result<StahlVal> {
    match arg {
        StahlVal::IntV(i) => (*i as f64).sin(),
        StahlVal::BigNum(i) => i.to_f64().unwrap().sin(),
        StahlVal::NumV(n) => n.sin(),
        StahlVal::Rational(r) => (*r.numer() as f32 / *r.denom() as f32).sin() as f64,
        _ => stop!(TypeMismatch => "sin expects a number, found: {}", arg),
    }
    .into_stahlval()
}

/// Returns the cosine value of the input angle, measured in radians.
///
/// (cos n) -> number?
///
/// * n : number? - The input angle, in radians.
///
/// # Examples
/// ```scheme
/// > (cos 0) ;; => 1
/// > (cos 1) ;; => 0.5403023058681398
/// > (cos 2.0) ;; => -0.4161468365471424
/// > (cos 3.14) ;; => -0.9999987317275395
/// ```
#[stahl_derive::function(name = "cos", constant = true)]
pub fn cos(arg: &StahlVal) -> Result<StahlVal> {
    match arg {
        StahlVal::IntV(i) => (*i as f64).cos(),
        StahlVal::BigNum(i) => i.to_f64().unwrap().cos(),
        StahlVal::NumV(n) => n.cos(),
        StahlVal::Rational(r) => (*r.numer() as f32 / *r.denom() as f32).cos() as f64,
        _ => stop!(TypeMismatch => "cos expects a number, found: {}", arg),
    }
    .into_stahlval()
}

/// Returns the tangent value of the input angle, measured in radians.
///
/// (tan n) -> number?
///
/// * n : number? - The input angle, in radians.
///
/// # Examples
/// ```scheme
/// > (tan 0) ;; => 0
/// > (tan 1) ;; => 1.557407724654902
/// > (tan 2.0) ;; => -2.185039863261519
/// > (tan 3.14) ;; => -0.0015926549364072232
/// ```
#[stahl_derive::function(name = "tan", constant = true)]
pub fn tan(arg: &StahlVal) -> Result<StahlVal> {
    match arg {
        StahlVal::IntV(i) => (*i as f64).tan(),
        StahlVal::BigNum(i) => i.to_f64().unwrap().tan(),
        StahlVal::NumV(n) => n.tan(),
        StahlVal::Rational(r) => (*r.numer() as f32 / *r.denom() as f32).tan() as f64,
        _ => stop!(TypeMismatch => "tan expects a number, found: {}", arg),
    }
    .into_stahlval()
}

/// Returns the arcsine, or inverse sine, of a value; output is in radians.
///
/// (asin n) -> number?
///
/// * n : number? - The input value is the sine of the angle you want and must be from -1 to 1.
///
/// # Examples
/// ```scheme
/// > (asin -1) ;; => -1.5707963267948966
/// > (asin 0) ;; => 0
/// > (asin 0.5) ;; => 0.5235987755982988
/// > (asin 2) ;; => +nan.0
/// ```
#[stahl_derive::function(name = "asin", constant = true)]
pub fn asin(arg: &StahlVal) -> Result<StahlVal> {
    match arg {
        StahlVal::IntV(i) => (*i as f64).asin(),
        StahlVal::BigNum(i) => i.to_f64().unwrap().asin(),
        StahlVal::NumV(n) => n.asin(),
        StahlVal::Rational(r) => (*r.numer() as f32 / *r.denom() as f32).asin() as f64,
        _ => stop!(TypeMismatch => "asin expects a number, found: {}", arg),
    }
    .into_stahlval()
}

/// Returns the arccosine, or inverse cosine, of a value; output is in radians.
///
/// (acos n) -> number?
///
/// * n : number? - The input value is the cosine of the angle you want and must be from -1 to 1.
///
/// # Examples
/// ```scheme
/// > (acos -1) ;; => 3.141592653589793
/// > (acos 0) ;; => 1.5707963267948966
/// > (acos 0.5) ;; => 1.0471975511965976
/// > (acos 2) ;; => +nan.0
/// ```
#[stahl_derive::function(name = "acos", constant = true)]
pub fn acos(arg: &StahlVal) -> Result<StahlVal> {
    match arg {
        StahlVal::IntV(i) => (*i as f64).acos(),
        StahlVal::BigNum(i) => i.to_f64().unwrap().acos(),
        StahlVal::NumV(n) => n.acos(),
        StahlVal::Rational(r) => (*r.numer() as f32 / *r.denom() as f32).acos() as f64,
        _ => stop!(TypeMismatch => "acos expects a number, found: {}", arg),
    }
    .into_stahlval()
}

/// Returns the arctangent, or inverse tangent, of a value; output is in radians.
///
/// (atan n) -> number?
///
/// * n : number? - The input value is the tangent of the angle you want.
///
/// # Examples
/// ```scheme
/// > (atan -1) ;; => -0.7853981633974483
/// > (atan 0) ;; => 0
/// > (atan 0.5) ;; => 0.46364760900080615
/// > (atan 2) ;; => 1.1071487177940906
/// ```
#[stahl_derive::function(name = "atan", constant = true)]
pub fn atan(arg: &StahlVal) -> Result<StahlVal> {
    match arg {
        StahlVal::IntV(i) => (*i as f64).atan(),
        StahlVal::BigNum(i) => i.to_f64().unwrap().atan(),
        StahlVal::NumV(n) => n.atan(),
        StahlVal::Rational(r) => (*r.numer() as f32 / *r.denom() as f32).atan() as f64,
        _ => stop!(TypeMismatch => "atan expects a number, found: {}", arg),
    }
    .into_stahlval()
}

/// Divides the given numbers.
///
/// (/ . nums) -> number?
///
/// * nums : number? - The numbers to divide. Must have at least one number.
///
/// # Examples
/// ```scheme
/// > (/ 10 2) ;; => 5
/// > (/ 10 2 2.0) ;; => 2.5
/// > (/ 1 3.0) ;; => 0.3333333333333333
/// > (/ 1 3) ;; => 1/3
/// ```
#[stahl_derive::native(name = "/", constant = true, arity = "AtLeast(1)")]
pub fn divide_primitive(args: &[StahlVal]) -> Result<StahlVal> {
    ensure_args_are_numbers("/", args)?;
    let recip = |x: &StahlVal| -> Result<StahlVal> {
        match x {
            StahlVal::IntV(n) => match i32::try_from(*n) {
                Ok(0) => {
                    stop!(Generic => "/: division by zero")
                }
                Ok(n) => Rational32::new(1, n).into_stahlval(),
                Err(_) => BigRational::new(BigInt::from(1), BigInt::from(*n)).into_stahlval(),
            },
            StahlVal::NumV(n) => n.recip().into_stahlval(),
            StahlVal::Rational(r) => r.recip().into_stahlval(),
            StahlVal::BigRational(r) => r.recip().into_stahlval(),
            StahlVal::BigNum(n) => BigRational::new(1.into(), n.as_ref().clone()).into_stahlval(),
            StahlVal::Complex(c) => complex_reciprocal(c),
            unexpected => {
                stahlerr!(TypeMismatch => "/ expects a number, but found: {:?}", unexpected)
            }
        }
    };
    match &args {
        [] => stahlerr!(ArityMismatch => "/ requires at least one argument"),
        [x] => recip(x),
        // TODO: Provide custom implementation to optimize by joining the multiply and recip calls.
        [x, y] => multiply_two(x, &recip(y)?),
        [x, ys @ ..] => {
            let d = multiply_primitive_impl(ys)?;
            multiply_two(&x, &recip(&d)?)
        }
    }
}

/// Checks if the given value is exact.
///
/// (exact? val) -> boolean?
///
/// * val : any - The value to check for exactness.
///
/// # Examples
/// ```scheme
/// > (exact? 42) ;; => #t
/// > (exact? 3.14) ;; => #f
/// > (exact? "hello") ;; => #f
/// ```
#[stahl_derive::function(name = "exact?", constant = true)]
pub fn exactp(value: &StahlVal) -> bool {
    match value {
        StahlVal::IntV(_)
        | StahlVal::BigNum(_)
        | StahlVal::Rational(_)
        | StahlVal::BigRational(_) => true,
        StahlVal::Complex(x) => exactp(&x.re) && exactp(&x.im),
        _ => false,
    }
}

/// Returns an exact representation of the input number, coerces an inexact number to an exact form.
///
/// (exact n) -> number?
///
/// * n : number? - The value to check for exactness.
///
/// # Examples
/// ```scheme
/// > (exact 5.0) ;; => 5
/// > (exact 5/3) ;; => 5/3
/// > (exact 2) ;; => 2
/// ```
#[stahl_derive::function(name = "exact", constant = true)]
pub fn exact(value: &StahlVal) -> Result<StahlVal> {
    match value {
        StahlVal::IntV(_)
        | StahlVal::BigNum(_)
        | StahlVal::Rational(_)
        | StahlVal::BigRational(_) => Ok(value.clone()),
        StahlVal::NumV(n) if n.fract() == 0.0 => Ok(StahlVal::IntV(*n as isize)),
        _ => stop!(Generic => "unable to convert to exact number: {}", value),
    }
}

/// Checks if the given value is inexact.
///
/// (inexact? val) -> boolean?
///
/// * val : any - The value to check for inexactness.
///
/// # Examples
/// ```scheme
/// > (inexact? 42) ;; => #f
/// > (inexact? 3.14) ;; => #t
/// ```
#[stahl_derive::function(name = "inexact?", constant = true)]
pub fn inexactp(value: &StahlVal) -> bool {
    match value {
        StahlVal::NumV(_) => true,
        StahlVal::Complex(x) => inexactp(&x.re) || inexactp(&x.im),
        _ => false,
    }
}

fn number_to_float(number: &StahlVal) -> Result<f64> {
    let res = match number {
        StahlVal::IntV(i) => *i as f64,
        StahlVal::Rational(f) => f.to_f64().unwrap(),
        StahlVal::BigRational(f) => f.to_f64().unwrap(),
        StahlVal::NumV(n) => *n,
        StahlVal::BigNum(n) => n.to_f64().unwrap(),
        _ => stop!(TypeMismatch => "number->float expects a real number, found: {}", number),
    };
    Ok(res)
}

/// Converts an exact number to an inexact number.
///
/// (exact->inexact num) -> number?
///
/// * num : number? - The number to convert from exact to inexact.
///
/// # Examples
/// ```scheme
/// > (exact->inexact 10) ;; => 10
/// > (exact->inexact 1/2) ;; => 0.5
/// > (exact->inexact 1+2i) ;; => 1+2i
/// ```
#[stahl_derive::function(name = "exact->inexact", constant = true)]
fn exact_to_inexact(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::IntV(i) => (*i as f64).into_stahlval(),
        StahlVal::Rational(f) => f.to_f64().unwrap().into_stahlval(),
        StahlVal::BigRational(f) => f.to_f64().unwrap().into_stahlval(),
        StahlVal::NumV(n) => n.into_stahlval(),
        StahlVal::BigNum(n) => Ok(StahlVal::NumV(n.to_f64().unwrap())),
        StahlVal::Complex(x) => {
            StahlComplex::new(exact_to_inexact(&x.re)?, exact_to_inexact(&x.im)?).into_stahlval()
        }
        _ => stahlerr!(TypeMismatch => "exact->inexact expects a number type, found: {}", number),
    }
}

/// Converts an inexact number to an exact number.
///
/// (inexact->exact num) -> number?
///
/// * num : number? - The number to convert from inexact to exact.
///
/// # Examples
/// ```scheme
/// > (inexact->exact 10.0) ;; => 10
/// > (inexact->exact 1.5) ;; => 3/2
/// > (inexact->exact 1.5+2.5i) ;; => 3/2+5/2i
/// ```
#[stahl_derive::function(name = "inexact->exact", constant = true)]
fn inexact_to_exact(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::IntV(x) => x.into_stahlval(),
        StahlVal::Rational(x) => x.into_stahlval(),
        StahlVal::BigRational(x) => StahlVal::BigRational(x.clone()).into_stahlval(),
        StahlVal::NumV(x) => {
            let x_isize = *x as isize;
            if x_isize as f64 == *x {
                return x_isize.into_stahlval();
            }
            BigRational::from_float(*x).into_stahlval()
        }
        StahlVal::BigNum(x) => StahlVal::BigNum(x.clone()).into_stahlval(),
        StahlVal::Complex(x) => {
            StahlComplex::new(inexact_to_exact(&x.re)?, inexact_to_exact(&x.im)?).into_stahlval()
        }
        _ => stahlerr!(TypeMismatch => "exact->inexact expects a number type, found: {}", number),
    }
}

fn finitep_impl(number: &StahlVal) -> Result<bool> {
    match number {
        StahlVal::NumV(x) if x.is_nan() || x.is_infinite() => Ok(false),
        StahlVal::IntV(_)
        | StahlVal::NumV(_)
        | StahlVal::BigNum(_)
        | StahlVal::Rational(_)
        | StahlVal::BigRational(_) => Ok(true),
        StahlVal::Complex(x) => Ok(finitep_impl(&x.re)? && finitep_impl(&x.im)?),
        _ => stahlerr!(TypeMismatch => "finite? expects a number, found: {}", number),
    }
}

/// Returns `#t` if the given number is finite.
///
/// (finite? number) -> boolean?
///
/// * number : number? - The number to check for finiteness.
///
/// # Examples
/// ```scheme
/// > (finite? 42) ;; => #t
/// > (finite? 0.1) ;; => #t
/// > (finite? +inf.0) ;; => #f
/// > (finite? -inf.0) ;; => #f
/// > (finite? +nan.0) ;; => #f
/// ```
#[stahl_derive::function(name = "finite?", constant = true)]
fn finitep(number: &StahlVal) -> Result<StahlVal> {
    finitep_impl(number).into_stahlval()
}

fn infinitep_impl(number: &StahlVal) -> Result<bool> {
    match number {
        StahlVal::NumV(x) if x.is_infinite() => Ok(true),
        StahlVal::IntV(_)
        | StahlVal::NumV(_)
        | StahlVal::BigNum(_)
        | StahlVal::Rational(_)
        | StahlVal::BigRational(_) => Ok(false),
        StahlVal::Complex(n) => Ok(infinitep_impl(&n.re)? || infinitep_impl(&n.im)?),
        _ => stahlerr!(TypeMismatch => "exact->inexact expects a real number, found: {}", number),
    }
}

/// Returns `#t` if the given number is infinite.
///
/// (infinite? number) -> boolean?
///
/// * number : number? - The number to check for infiniteness.
///
/// # Examples
/// ```scheme
/// > (infinite? 42) ;; => #f
/// > (infinite? -nan.0) ;; => #f
/// > (infinite? +inf.0) ;; => #t
/// ```
#[stahl_derive::function(name = "infinite?", constant = true)]
fn infinitep(number: &StahlVal) -> Result<StahlVal> {
    infinitep_impl(number)?.into_stahlval()
}

/// Computes the absolute value of the given number.
///
/// (abs number) -> number?
///
/// * number : number? - The number to compute the absolute value of.
///
/// # Examples
/// ```scheme
/// > (abs 42) ;; => 42
/// > (abs -42) ;; => 42
/// > (abs 0) ;; => 0
/// ```
#[stahl_derive::function(name = "abs", constant = true)]
fn abs(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::IntV(i) => Ok(StahlVal::IntV(i.abs())),
        StahlVal::NumV(n) => Ok(StahlVal::NumV(n.abs())),
        StahlVal::Rational(f) => f.abs().into_stahlval(),
        StahlVal::BigRational(f) => f.abs().into_stahlval(),
        StahlVal::BigNum(n) => n.as_ref().abs().into_stahlval(),
        _ => stahlerr!(TypeMismatch => "abs expects a real number, found: {}", number),
    }
}

/// Rounds the given number up to the nearest integer not less than it.
///
/// (ceiling number) -> integer?
///
/// * number : number? - The number to round up.
///
/// # Examples
/// ```scheme
/// > (ceiling 42) ;; => 42
/// > (ceiling 42.1) ;; => 43
/// > (ceiling -42.1) ;; => -42
/// ```
#[stahl_derive::function(name = "ceiling", constant = true)]
fn ceiling(number: &StahlVal) -> Result<StahlVal> {
    match number {
        n @ StahlVal::IntV(_) | n @ StahlVal::BigNum(_) => Ok(n.clone()),
        StahlVal::NumV(n) => Ok(StahlVal::NumV(n.ceil())),
        StahlVal::Rational(f) => f.ceil().into_stahlval(),
        StahlVal::BigRational(f) => f.ceil().into_stahlval(),
        _ => stahlerr!(TypeMismatch => "ceiling expects a real number, found: {}", number),
    }
}

/// Retrieves the denominator of the given rational number.
///
/// (denominator number) -> integer?
///
/// * number : number? - The rational number to retrieve the denominator from.
///
/// # Examples
/// ```scheme
/// > (denominator 1/2) ;; => 2
/// > (denominator 3/4) ;; => 4
/// > (denominator 4) ;; => 1
/// ```
#[stahl_derive::function(name = "denominator", constant = true)]
fn denominator(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::IntV(_) | StahlVal::BigNum(_) => 1.into_stahlval(),
        StahlVal::NumV(_) => {
            stahlerr!(TypeMismatch => "denominator not supported for number {}", number)
        }
        StahlVal::Rational(f) => f.denom().into_stahlval(),
        StahlVal::BigRational(f) => f.denom().clone().into_stahlval(),
        _ => stahlerr!(TypeMismatch => "ceiling expects a real number, found: {}", number),
    }
}

// TODO: Add support for BigNum.
/// Raises the left operand to the power of the right operand.
///
/// (expt base exponent) -> number?
///
/// * base : number? - The base number.
/// * exponent : number? - The exponent to raise the base to.
///
/// # Examples
/// ```scheme
/// > (expt 2 3) ;; => 8
/// > (expt 2.0 0.5) ;; => 1.4142135623730951
/// > (expt 9 0.5) ;; => 3
/// ```
#[stahl_derive::function(name = "expt", constant = true)]
fn expt(left: &StahlVal, right: &StahlVal) -> Result<StahlVal> {
    match (left, right) {
        (StahlVal::IntV(l), StahlVal::IntV(r)) if *r >= 0 => {
            match u32::try_from(*r).ok().and_then(|r| l.checked_pow(r)) {
                Some(val) => val.into_stahlval(),
                None => BigInt::from(*l).pow(*r as usize).into_stahlval(),
            }
        }
        // r is negative here
        (StahlVal::IntV(l), StahlVal::IntV(r)) => {
            if l.is_zero() {
                stop!(Generic => "expt: 0 cannot be raised to a negative power");
            }

            let r = r.unsigned_abs();
            // a^-b = 1/(a^b)
            match (u32::try_from(r).ok())
                .and_then(|r| l.checked_pow(r))
                .and_then(|l| i32::try_from(l).ok())
            {
                Some(val) => Rational32::new_raw(1, val).into_stahlval(),
                None => {
                    BigRational::new_raw(BigInt::from(1), BigInt::from(*l).pow(r)).into_stahlval()
                }
            }
        }
        (StahlVal::IntV(l), StahlVal::NumV(r)) => (*l as f64).powf(*r).into_stahlval(),
        (StahlVal::IntV(l), StahlVal::Rational(r)) => {
            (*l as f64).powf(r.to_f64().unwrap()).into_stahlval()
        }
        (StahlVal::IntV(l), StahlVal::BigNum(r)) => {
            if l.is_zero() {
                stop!(Generic => "expt: 0 cannot be raised to a negative power");
            }

            let expt = BigInt::from(*l).pow(r.magnitude());
            match r.sign() {
                num::bigint::Sign::Plus | num::bigint::Sign::NoSign => expt.into_stahlval(),
                num::bigint::Sign::Minus => {
                    BigRational::new_raw(BigInt::from(1), expt).into_stahlval()
                }
            }
        }
        (StahlVal::IntV(l), StahlVal::BigRational(r)) => {
            (*l as f64).powf(r.to_f64().unwrap()).into_stahlval()
        }
        (StahlVal::NumV(l), StahlVal::NumV(r)) => Ok(StahlVal::NumV(l.powf(*r))),
        (StahlVal::NumV(l), StahlVal::IntV(r)) => match i32::try_from(*r) {
            Ok(r) => l.powi(r).into_stahlval(),
            Err(_) => l.powf(*r as f64).into_stahlval(),
        },
        (StahlVal::NumV(l), StahlVal::Rational(r)) => l.powf(r.to_f64().unwrap()).into_stahlval(),
        (StahlVal::NumV(l), StahlVal::BigNum(r)) => l.powf(r.to_f64().unwrap()).into_stahlval(),
        (StahlVal::NumV(l), StahlVal::BigRational(r)) => {
            l.powf(r.to_f64().unwrap()).into_stahlval()
        }
        (StahlVal::Rational(l), StahlVal::Rational(r)) => l
            .to_f64()
            .unwrap()
            .powf(r.to_f64().unwrap())
            .into_stahlval(),
        (StahlVal::Rational(l), StahlVal::NumV(r)) => l.to_f64().unwrap().powf(*r).into_stahlval(),
        (StahlVal::Rational(l), StahlVal::IntV(r)) => match i32::try_from(*r) {
            Ok(r) => l.pow(r).into_stahlval(),
            Err(_) => {
                let base = BigRational::new(BigInt::from(*l.numer()), BigInt::from(*l.denom()));
                let exp = BigInt::from(*r);
                base.pow(exp).into_stahlval()
            }
        },
        (StahlVal::Rational(l), StahlVal::BigNum(r)) => l
            .to_f64()
            .unwrap()
            .powf(r.to_f64().unwrap())
            .into_stahlval(),
        (StahlVal::Rational(l), StahlVal::BigRational(r)) => l
            .to_f64()
            .unwrap()
            .powf(r.to_f64().unwrap())
            .into_stahlval(),
        (StahlVal::BigNum(l), StahlVal::BigNum(r)) => {
            let expt = l.as_ref().clone().pow(r.magnitude());
            match r.sign() {
                num::bigint::Sign::NoSign | num::bigint::Sign::Plus => expt.into_stahlval(),
                num::bigint::Sign::Minus => {
                    BigRational::new_raw(BigInt::from(1), expt).into_stahlval()
                }
            }
        }
        (StahlVal::BigNum(l), StahlVal::IntV(r)) => match *r {
            0 => 1.into_stahlval(),
            r if r < 0 => {
                BigRational::new_raw(BigInt::from(1), l.as_ref().clone().pow(r.unsigned_abs()))
                    .into_stahlval()
            }
            r => l.as_ref().clone().pow(r as usize).into_stahlval(),
        },
        (StahlVal::BigNum(l), StahlVal::NumV(r)) => l.to_f64().unwrap().powf(*r).into_stahlval(),
        (StahlVal::BigNum(l), StahlVal::Rational(r)) => l
            .to_f64()
            .unwrap()
            .powf(r.to_f64().unwrap())
            .into_stahlval(),
        (StahlVal::BigNum(l), StahlVal::BigRational(r)) => l
            .to_f64()
            .unwrap()
            .powf(r.to_f64().unwrap())
            .into_stahlval(),
        (StahlVal::BigRational(l), StahlVal::Rational(r)) => l
            .to_f64()
            .unwrap()
            .powf(r.to_f64().unwrap())
            .into_stahlval(),
        (StahlVal::BigRational(l), StahlVal::NumV(r)) => {
            l.to_f64().unwrap().powf(*r).into_stahlval()
        }
        (StahlVal::BigRational(l), StahlVal::IntV(r)) => match i32::try_from(*r) {
            Ok(r) => l.as_ref().pow(r).into_stahlval(),
            Err(_) => {
                let exp = BigInt::from(*r);
                l.as_ref().clone().pow(exp).into_stahlval()
            }
        },
        (StahlVal::BigRational(l), StahlVal::BigNum(r)) => l
            .to_f64()
            .unwrap()
            .powf(r.to_f64().unwrap())
            .into_stahlval(),
        (StahlVal::BigRational(l), StahlVal::BigRational(r)) => l
            .to_f64()
            .unwrap()
            .powf(r.to_f64().unwrap())
            .into_stahlval(),
        (l, r) => {
            stahlerr!(TypeMismatch => "expt expected two numbers but found {} and {}", l, r)
        }
    }
}

/// Returns Euler’s number raised to the power of z.
///
/// (exp z) -> number?
///
/// * z : number? - The number to raise e to the power of.
///
/// # Examples
/// ```scheme
/// > (exp 0) ;; => 1
/// > (exp 2) ;; => 7.38905609893065
/// > (exp 1.5) ;; => 4.4816890703380645
/// ```
#[stahl_derive::function(name = "exp", constant = true)]
fn exp(left: &StahlVal) -> Result<StahlVal> {
    match left {
        StahlVal::IntV(0) => Ok(StahlVal::IntV(1)),
        StahlVal::IntV(l) if *l < i32::MAX as isize => {
            Ok(StahlVal::NumV(std::f64::consts::E.powi(*l as i32)))
        }
        maybe_number => match number_to_float(maybe_number) {
            Ok(n) => Ok(StahlVal::NumV(std::f64::consts::E.powf(n))),
            Err(_) => stahlerr!(Generic => "exp expected a real number"),
        },
    }
}

/// Computes the largest integer less than or equal to the given number.
///
/// (floor number) -> number?
///
/// * number : number? - The number to compute the floor for.
///
/// # Examples
/// ```scheme
/// > (floor 3.14) ;; => 3
/// > (floor 4.99) ;; => 4
/// > (floor -2.5) ;; => -3
/// ```
#[stahl_derive::function(name = "floor", constant = true)]
fn floor(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::NumV(x) => Ok(StahlVal::NumV(x.floor())),
        StahlVal::IntV(x) => x.into_stahlval(),
        StahlVal::Rational(x) => x.floor().into_stahlval(),
        StahlVal::BigNum(x) => Ok(StahlVal::BigNum(x.clone())),
        StahlVal::BigRational(x) => x.floor().into_stahlval(),
        _ => stahlerr!(Generic => "floor expected a real number"),
    }
}

/// Retrieves the numerator of the given rational number.
///
/// (numerator number) -> number?
///
/// * number : number? - The rational number to retrieve the numerator from.
///
/// # Examples
/// ```scheme
/// > (numerator 3/4) ;; => 3
/// > (numerator 5/2) ;; => 5
/// > (numerator -2) ;; => -2
/// ```
#[stahl_derive::function(name = "numerator", constant = true)]
fn numerator(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::IntV(x) => x.into_stahlval(),
        StahlVal::Rational(x) => (*x.numer() as isize).into_stahlval(),
        StahlVal::BigNum(x) => Ok(StahlVal::BigNum(x.clone())),
        StahlVal::BigRational(x) => (x.numer().clone()).into_stahlval(),
        _ => stahlerr!(Generic => "numerator expects an integer or rational number"),
    }
}

/// Rounds the given number to the nearest integer.
///
/// (round number) -> number?
///
/// * number : number? - The number to round.
///
/// # Examples
/// ```scheme
/// > (round 3.14) ;; => 3
/// > (round 4.6) ;; => 5
/// > (round -2.5) ;; => -3
/// ```
#[stahl_derive::function(name = "round", constant = true)]
fn round(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::IntV(i) => i.into_stahlval(),
        StahlVal::NumV(n) => n.round().into_stahlval(),
        StahlVal::Rational(f) => f.round().into_stahlval(),
        StahlVal::BigRational(f) => f.round().into_stahlval(),
        StahlVal::BigNum(n) => Ok(StahlVal::BigNum(n.clone())),
        _ => stahlerr!(TypeMismatch => "round expects a real number, found: {}", number),
    }
}

/// Computes the square of the given number.
///
/// (square number) -> number?
///
/// * number : number? - The number to square.
///
/// # Examples
/// ```scheme
/// > (square 5) ;; => 25
/// > (square -3) ;; => 9
/// > (square 2.5) ;; => 6.25
/// ```
#[stahl_derive::function(name = "square", constant = true)]
fn square(number: &StahlVal) -> Result<StahlVal> {
    if !numberp(number) {
        stop!(TypeMismatch => "square expects a number, found: {:?}", number)
    }
    multiply_two(&number, &number)
}

/// Computes the square root of the given number.
///
/// (sqrt number) -> number?
///
/// * number : number? - The number to compute the square root for.
///
/// # Examples
/// ```scheme
/// > (sqrt 4) ;; => 2
/// > (sqrt 2) ;; => 1.4142135623730951
/// > (sqrt -1) ;; => 0+1i
/// ```
#[stahl_derive::function(name = "sqrt", constant = true)]
fn sqrt(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::NumV(x) => {
            if x.is_negative() {
                let imag = x.neg().sqrt();
                StahlComplex::new(0.0.into_stahlval()?, imag.into_stahlval()?).into_stahlval()
            } else {
                x.sqrt().into_stahlval()
            }
        }
        StahlVal::IntV(x) => {
            if x.is_negative() {
                let sqrt = (*x as f64).abs().sqrt();
                if sqrt as isize as f64 == sqrt {
                    StahlComplex::new(0.into_stahlval()?, (sqrt as isize).into_stahlval()?)
                        .into_stahlval()
                } else {
                    StahlComplex::new(0.into_stahlval()?, sqrt.into_stahlval()?).into_stahlval()
                }
            } else {
                let sqrt = (*x as f64).sqrt();
                if sqrt as isize as f64 == sqrt {
                    (sqrt as isize).into_stahlval()
                } else {
                    sqrt.into_stahlval()
                }
            }
        }
        StahlVal::Rational(x) => {
            let n = x.numer().abs();
            let d = *x.denom();
            let n_sqrt = (n as f64).sqrt();
            let d_sqrt = (d as f64).sqrt();
            let sqrt = if n_sqrt as i32 as f64 == n_sqrt && d_sqrt as i32 as f64 == d_sqrt {
                Rational32::new(n_sqrt as i32, d_sqrt as i32).into_stahlval()?
            } else {
                (n_sqrt / d_sqrt).into_stahlval()?
            };
            if x.is_negative() {
                let re = if exactp(&sqrt) {
                    0.into_stahlval()?
                } else {
                    0.0.into_stahlval()?
                };
                StahlComplex::new(re, sqrt).into_stahlval()
            } else {
                Ok(sqrt)
            }
        }
        StahlVal::BigNum(n) => {
            let sqrt = n.as_ref().to_f64().unwrap().sqrt();
            if n.as_ref().is_negative() {
                StahlComplex::new(0.0.into_stahlval()?, sqrt.into_stahlval()?).into_stahlval()
            } else {
                sqrt.into_stahlval()
            }
        }
        StahlVal::BigRational(n) => {
            let sqrt = n.as_ref().to_f64().unwrap().sqrt();
            if n.as_ref().is_negative() {
                StahlComplex::new(0.0.into_stahlval()?, sqrt.into_stahlval()?).into_stahlval()
            } else {
                sqrt.into_stahlval()
            }
        }
        StahlVal::Complex(n) => {
            let z_mag = magnitude(number)?;
            let half = Rational32::new(1, 2).into_stahlval()?;
            let re = sqrt(&multiply_two(&add_two(&z_mag, &n.re)?, &half)?)?;
            let im = sqrt(&multiply_two(&add_two(&z_mag, &negate(&n.re)?)?, &half)?)?;
            if negativep(&n.im)? == StahlVal::BoolV(true) {
                StahlComplex::new(re, negate(&im)?).into_stahlval()
            } else {
                StahlComplex::new(re, im).into_stahlval()
            }
        }
        _ => stahlerr!(TypeMismatch => "sqrt expected a number"),
    }
}

/// Returns the real part of a number
///
/// (real-part number) -> number?
///
/// # Examples
/// ```scheme
/// > (real-part 3+4i) ;; => 3
/// > (real-part 42) ;; => 42
/// ```
#[stahl_derive::function(name = "real-part", constant = true)]
pub fn real_part(value: &StahlVal) -> Result<StahlVal> {
    match value {
        val @ StahlVal::IntV(_)
        | val @ StahlVal::BigNum(_)
        | val @ StahlVal::Rational(_)
        | val @ StahlVal::BigRational(_)
        | val @ StahlVal::NumV(_) => Ok(val.clone()),
        StahlVal::Complex(complex) => Ok(complex.re.clone()),
        _ => stahlerr!(TypeMismatch => "real-part expected number"),
    }
}

/// Returns the imaginary part of a number
///
/// (imag-part number) -> number?
///
/// # Examples
/// ```scheme
/// > (imag-part 3+4i) ;; => 4
/// > (imag-part 42) ;; => 0
/// ```
#[stahl_derive::function(name = "imag-part", constant = true)]
pub fn imag_part(value: &StahlVal) -> Result<StahlVal> {
    match value {
        StahlVal::IntV(_)
        | StahlVal::BigNum(_)
        | StahlVal::Rational(_)
        | StahlVal::BigRational(_)
        | StahlVal::NumV(_) => Ok(StahlVal::IntV(0)),
        StahlVal::Complex(complex) => Ok(complex.im.clone()),
        _ => stahlerr!(TypeMismatch => "imag-part expected number"),
    }
}

/// Computes the magnitude of the given number.
///
/// (magnitude number) -> number?
///
/// * number : number? - The number to compute the magnitude for.
///
/// # Examples
/// ```scheme
/// > (magnitude 3+4i) ;; => 5
/// > (magnitude 5) ;; => 5
/// > (magnitude -5) ;; => 5
/// ```
#[stahl_derive::function(name = "magnitude", constant = true)]
fn magnitude(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::NumV(x) => x.abs().into_stahlval(),
        StahlVal::IntV(x) => x.abs().into_stahlval(),
        StahlVal::Rational(x) => x.abs().into_stahlval(),
        StahlVal::BigNum(x) => x.as_ref().abs().into_stahlval(),
        StahlVal::BigRational(x) => x.as_ref().abs().into_stahlval(),
        StahlVal::Complex(x) => {
            let c_squared = add_two(&square(&x.re)?, &square(&x.im)?)?;
            sqrt(&c_squared)
        }
        _ => stahlerr!(TypeMismatch => "magnitude expects a number, found {number}"),
    }
}

/// Computes the natural logarithm of the given number.
///
/// (log number [base]) -> number?
///
/// * number : number? - The number to compute the logarithm for.
/// * base : number? - The base of the logarithm. If not provided, defaults to Euler's number (e).
///
/// # Examples
/// ```scheme
/// > (log 10) ;; => 2.302585092994046
/// > (log 100 10) ;; => 2
/// > (log 27 3) ;; => 3
/// ```
#[stahl_derive::native(name = "log", arity = "AtLeast(1)")]
fn log(args: &[StahlVal]) -> Result<StahlVal> {
    if args.len() > 2 {
        stop!(ArityMismatch => "log expects one or two arguments, found: {}", args.len());
    }

    let first = &args[0];
    let base = args
        .get(1)
        .cloned()
        .unwrap_or(StahlVal::NumV(std::f64::consts::E));

    match (first, &base) {
        (StahlVal::IntV(1), _) => Ok(StahlVal::IntV(0)),
        (StahlVal::IntV(_) | StahlVal::NumV(_), StahlVal::IntV(1)) => {
            stahlerr!(Generic => "log: divide by zero with args: {} and {}", first, base)
        }
        (StahlVal::IntV(arg), StahlVal::NumV(n)) => Ok(StahlVal::NumV((*arg as f64).log(*n))),
        (StahlVal::IntV(arg), StahlVal::IntV(base)) => Ok(StahlVal::IntV(arg.ilog(*base) as isize)),
        (StahlVal::NumV(arg), StahlVal::NumV(n)) => Ok(StahlVal::NumV(arg.log(*n))),
        (StahlVal::NumV(arg), StahlVal::IntV(base)) => Ok(StahlVal::NumV(arg.log(*base as f64))),
        // TODO: Support BigNum, Rational, and BigRational.
        _ => {
            stahlerr!(TypeMismatch => "log expects one or two numbers, found: {} and {}", first, base)
        }
    }
}

/// Computes the integer square root of the given non-negative integer.
///
/// (exact-integer-sqrt number) -> (integer? integer?)
///
/// * number : (and/c integer? positive?) - The non-negative integer to compute the square root for.
///
/// # Examples
/// ```scheme
/// > (exact-integer-sqrt 25) ;; => (5 0)
/// > (exact-integer-sqrt 35) ;; => (5 10)
/// ```
#[stahl_derive::function(name = "exact-integer-sqrt", constant = true)]
fn exact_integer_sqrt(number: &StahlVal) -> Result<StahlVal> {
    match number {
        StahlVal::IntV(x) if *x >= 0 => {
            let (ans, rem) = exact_integer_impl(x);
            (ans.into_stahlval()?, rem.into_stahlval()?).into_stahlval()
        }
        StahlVal::BigNum(x) if !x.is_negative() => {
            let (ans, rem) = exact_integer_impl(x.as_ref());
            (ans.into_stahlval()?, rem.into_stahlval()?).into_stahlval()
        }
        _ => {
            stahlerr!(TypeMismatch => "exact-integer-sqrt expects a non-negative integer but found {number}")
        }
    }
}

fn exact_integer_impl<'a, N>(target: &'a N) -> (N, N)
where
    N: num::integer::Roots + Clone,
    &'a N: std::ops::Mul<&'a N, Output = N>,
    N: std::ops::Sub<N, Output = N>,
{
    let x = target.sqrt();
    let x_sq = x.clone() * x.clone();
    let rem = target.clone() - x_sq;
    (x, rem)
}

/// Performs a bitwise arithmetic shift using the given 2 numbers
///
/// (arithmetic-shift n m) -> integer?
///
/// * n : integer? - The number to shift.
/// * m : integer? - The number by which to shift.
///
/// # Examples
/// ```scheme
/// > (arithmetic-shift 10 1) ;; => 20
/// > (arithmetic-shift 20 1) ;; => 40
/// > (arithmetic-shift 40 -2) ;; => 10
/// ```
#[stahl_derive::native(name = "arithmetic-shift", constant = true, arity = "Exact(2)")]
pub fn arithmetic_shift(args: &[StahlVal]) -> Result<StahlVal> {
    match &args {
        [n, m] => match (n, m) {
            (StahlVal::IntV(n), StahlVal::IntV(m)) => {
                if *m >= 0 {
                    Ok(StahlVal::IntV(n << m))
                } else {
                    Ok(StahlVal::IntV(n >> -m))
                }
            }
            _ => stop!(TypeMismatch => "arithmetic-shift expected 2 integers"),
        },
        _ => stop!(ArityMismatch => "arithmetic-shift takes 2 arguments"),
    }
}

/// Checks if the given number is even
///
/// (even? n) -> bool?
///
/// * n : number? - The number to check for evenness.
///
/// # Examples
/// ```scheme
/// > (even? 2) ;; => #true
/// > (even? 3) ;; => #false
/// > (even? 4.0) ;; => #true
/// ```
#[stahl_derive::function(name = "even?", constant = true)]
pub fn even(arg: &StahlVal) -> Result<StahlVal> {
    match arg {
        StahlVal::IntV(n) => Ok(StahlVal::BoolV(n & 1 == 0)),
        StahlVal::BigNum(n) => Ok(StahlVal::BoolV(n.is_even())),
        StahlVal::NumV(n) if n.fract() == 0.0 => (*n as i64).is_even().into_stahlval(),
        _ => stahlerr!(TypeMismatch => "even? requires an integer, found: {:?}", arg),
    }
}

/// Checks if the given number is odd
///
/// (odd? n) -> bool?
///
/// * n : number? - The number to check for oddness.
///
/// # Examples
/// ```scheme
/// > (odd? 2) ;; => #false
/// > (odd? 3) ;; => #true
/// > (odd? 5.0) ;; => #true
/// ```
#[stahl_derive::function(name = "odd?", constant = true)]
pub fn odd(arg: &StahlVal) -> Result<StahlVal> {
    match arg {
        StahlVal::IntV(n) => Ok(StahlVal::BoolV(n & 1 == 1)),
        StahlVal::BigNum(n) => Ok(StahlVal::BoolV(n.is_odd())),
        StahlVal::NumV(n) if n.fract() == 0.0 => (*n as i64).is_odd().into_stahlval(),
        _ => {
            stahlerr!(TypeMismatch => "odd? requires an integer, found: {:?}", arg)
        }
    }
}

/// Sums all given floats
///
/// (f+ nums) -> number?
///
/// * nums : float? - The floats to sum up.
///
/// # Examples
/// ```scheme
/// > (f+ 5.5) ;; => 5.5
/// > (f+ 1.1 2.2) ;; => 3.3
/// > (f+ 3.3 3.3 3.3) ;; => 9.9
/// ```
#[stahl_derive::native(name = "f+", constant = true, arity = "AtLeast(1)")]
pub fn float_add(args: &[StahlVal]) -> Result<StahlVal> {
    if args.is_empty() {
        stop!(ArityMismatch => "f+ requires at least one argument")
    }
    let mut sum = 0.0;

    for arg in args {
        if let StahlVal::NumV(n) = arg {
            sum += n;
        } else {
            stop!(TypeMismatch => "f+ expected a float, found {:?}", arg);
        }
    }

    Ok(StahlVal::NumV(sum))
}

fn ensure_args_are_numbers(op: &str, args: &[StahlVal]) -> Result<()> {
    for arg in args {
        if !numberp(arg) {
            stop!(TypeMismatch => "{op} expects a number, found: {:?}", arg)
        }
    }
    Ok(())
}

/// Multiplies `x` and `y` without any type checking.
///
/// # Precondition
/// - `x` and `y` must be valid numerical types.
fn multiply_two(x: &StahlVal, y: &StahlVal) -> Result<StahlVal> {
    match (x, y) {
        (StahlVal::NumV(x), StahlVal::NumV(y)) => (x * y).into_stahlval(),
        (StahlVal::NumV(x), StahlVal::IntV(y)) | (StahlVal::IntV(y), StahlVal::NumV(x)) => {
            (x * *y as f64).into_stahlval()
        }
        (StahlVal::NumV(x), StahlVal::BigNum(y)) | (StahlVal::BigNum(y), StahlVal::NumV(x)) => {
            (x * y.to_f64().unwrap()).into_stahlval()
        }
        (StahlVal::NumV(x), StahlVal::Rational(y)) | (StahlVal::Rational(y), StahlVal::NumV(x)) => {
            (x * y.to_f64().unwrap()).into_stahlval()
        }
        (StahlVal::NumV(x), StahlVal::BigRational(y))
        | (StahlVal::BigRational(y), StahlVal::NumV(x)) => {
            (x * y.to_f64().unwrap()).into_stahlval()
        }
        (StahlVal::IntV(x), StahlVal::IntV(y)) => match x.checked_mul(y) {
            Some(res) => res.into_stahlval(),
            None => {
                let mut res = BigInt::from(*x);
                res *= *y;
                res.into_stahlval()
            }
        },
        (StahlVal::IntV(x), StahlVal::BigNum(y)) | (StahlVal::BigNum(y), StahlVal::IntV(x)) => {
            (y.as_ref() * x).into_stahlval()
        }
        (StahlVal::IntV(x), StahlVal::Rational(y)) | (StahlVal::Rational(y), StahlVal::IntV(x)) => {
            match i32::try_from(*x) {
                Ok(x) => match y.checked_mul(&Rational32::new(x, 1)) {
                    Some(res) => res.into_stahlval(),
                    None => {
                        let mut res =
                            BigRational::new(BigInt::from(*y.numer()), BigInt::from(*y.denom()));
                        res *= BigInt::from(x);
                        res.into_stahlval()
                    }
                },
                Err(_) => {
                    let mut res =
                        BigRational::new(BigInt::from(*y.numer()), BigInt::from(*y.denom()));
                    res *= BigInt::from(*x);
                    res.into_stahlval()
                }
            }
        }
        (StahlVal::IntV(x), StahlVal::BigRational(y))
        | (StahlVal::BigRational(y), StahlVal::IntV(x)) => {
            let mut res = y.as_ref().clone();
            res *= BigInt::from(*x);
            res.into_stahlval()
        }
        (StahlVal::Rational(x), StahlVal::Rational(y)) => match x.checked_mul(y) {
            Some(res) => res.into_stahlval(),
            None => {
                let mut res = BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()));
                res *= BigRational::new(BigInt::from(*y.numer()), BigInt::from(*y.denom()));
                res.into_stahlval()
            }
        },
        (StahlVal::Rational(x), StahlVal::BigNum(y))
        | (StahlVal::BigNum(y), StahlVal::Rational(x)) => {
            let mut res = BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()));
            res *= y.as_ref();
            res.into_stahlval()
        }
        (StahlVal::BigRational(x), StahlVal::BigRational(y)) => {
            (x.as_ref() * y.as_ref()).into_stahlval()
        }
        (StahlVal::BigRational(x), StahlVal::BigNum(y))
        | (StahlVal::BigNum(y), StahlVal::BigRational(x)) => {
            (x.as_ref() * y.as_ref()).into_stahlval()
        }
        (StahlVal::BigNum(x), StahlVal::BigNum(y)) => (x.as_ref() * y.as_ref()).into_stahlval(),
        // Complex numbers.
        (StahlVal::Complex(x), StahlVal::Complex(y)) => multiply_complex(x, y),
        (StahlVal::Complex(x), y) | (y, StahlVal::Complex(x)) => {
            let y = StahlComplex::new(y.clone(), StahlVal::IntV(0));
            multiply_complex(x, &y)
        }
        (StahlVal::BigRational(x), StahlVal::Rational(y)) => {
            let mut res = BigRational::new(
                BigInt::from(x.numer().clone()),
                BigInt::from(x.denom().clone()),
            );
            res *= BigRational::new(BigInt::from(*y.numer()), BigInt::from(*y.denom()));
            res.into_stahlval()
        }
        _ => unreachable!(),
    }
}

/// # Precondition
/// All types in `args` must be numerical types.
fn multiply_primitive_impl(args: &[StahlVal]) -> Result<StahlVal> {
    match args {
        [] => 1.into_stahlval(),
        [x] => x.clone().into_stahlval(),
        [x, y] => multiply_two(x, y).into_stahlval(),
        [x, y, zs @ ..] => {
            let mut res = multiply_two(x, y)?;
            for z in zs {
                // TODO: This use case could be optimized to reuse state instead of creating a new
                // object each time.
                res = multiply_two(&res, &z)?;
            }
            res.into_stahlval()
        }
    }
}

#[cold]
fn complex_reciprocal(c: &StahlComplex) -> Result<StahlVal> {
    let denominator = add_two(&multiply_two(&c.re, &c.re)?, &multiply_two(&c.im, &c.im)?)?;
    let re = divide_primitive(&[c.re.clone(), denominator.clone()])?;
    let neg_im = divide_primitive(&[c.re.clone(), denominator])?;
    StahlComplex::new(re, subtract_primitive(&[neg_im])?).into_stahlval()
}

/// Negate a number.
///
/// # Precondition
/// `value` must be a number.
#[inline(always)]
fn negate(value: &StahlVal) -> Result<StahlVal> {
    match value {
        StahlVal::NumV(x) => (-x).into_stahlval(),
        StahlVal::IntV(x) => match x.checked_neg() {
            Some(res) => res.into_stahlval(),
            None => BigInt::from(*x).neg().into_stahlval(),
        },
        StahlVal::Rational(x) => match 0i32.checked_sub(*x.numer()) {
            Some(n) => Rational32::new(n, *x.denom()).into_stahlval(),
            None => BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()))
                .neg()
                .into_stahlval(),
        },
        StahlVal::BigRational(x) => x.as_ref().neg().into_stahlval(),
        StahlVal::BigNum(x) => x.as_ref().clone().neg().into_stahlval(),
        StahlVal::Complex(x) => negate_complex(x),
        _ => unreachable!(),
    }
}

/// Adds two numbers.
///
/// # Precondition
/// x and y must be valid numbers.
#[inline(always)]
pub fn add_two(x: &StahlVal, y: &StahlVal) -> Result<StahlVal> {
    match (x, y) {
        // Simple integer case. Probably very common.
        (StahlVal::IntV(x), StahlVal::IntV(y)) => match x.checked_add(y) {
            Some(res) => res.into_stahlval(),
            None => {
                let mut res = BigInt::from(*x);
                res += *y;
                res.into_stahlval()
            }
        },
        // Cases that return an `f64`.
        (StahlVal::NumV(x), StahlVal::NumV(y)) => (x + y).into_stahlval(),
        (StahlVal::NumV(x), StahlVal::IntV(y)) | (StahlVal::IntV(y), StahlVal::NumV(x)) => {
            (x + *y as f64).into_stahlval()
        }
        (StahlVal::NumV(x), StahlVal::BigNum(y)) | (StahlVal::BigNum(y), StahlVal::NumV(x)) => {
            (x + y.to_f64().unwrap()).into_stahlval()
        }
        (StahlVal::NumV(x), StahlVal::Rational(y)) | (StahlVal::Rational(y), StahlVal::NumV(x)) => {
            (x + y.to_f64().unwrap()).into_stahlval()
        }
        (StahlVal::NumV(x), StahlVal::BigRational(y))
        | (StahlVal::BigRational(y), StahlVal::NumV(x)) => {
            (x + y.to_f64().unwrap()).into_stahlval()
        }
        // Cases that interact with `Rational`.
        (StahlVal::Rational(x), StahlVal::Rational(y)) => (x + y).into_stahlval(),
        (StahlVal::Rational(x), StahlVal::IntV(y)) | (StahlVal::IntV(y), StahlVal::Rational(x)) => {
            match i32::try_from(*y) {
                Ok(y) => match x.checked_add(&Rational32::new(y, 1)) {
                    Some(res) => res.into_stahlval(),
                    None => {
                        let res =
                            BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()))
                                * BigInt::from(y);
                        res.into_stahlval()
                    }
                },
                Err(_) => {
                    let res = BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()))
                        * BigInt::from(*y);
                    res.into_stahlval()
                }
            }
        }
        (StahlVal::Rational(x), StahlVal::BigNum(y))
        | (StahlVal::BigNum(y), StahlVal::Rational(x)) => {
            let res =
                BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom())) * y.as_ref();
            res.into_stahlval()
        }
        // Cases that interact with `BigRational`. For the sake of performance, hopefully not too
        // common.
        (StahlVal::BigRational(x), StahlVal::BigRational(y)) => {
            (x.as_ref() + y.as_ref()).into_stahlval()
        }
        (StahlVal::BigRational(x), StahlVal::IntV(y))
        | (StahlVal::IntV(y), StahlVal::BigRational(x)) => {
            (x.as_ref() + BigInt::from(*y)).into_stahlval()
        }
        (StahlVal::BigRational(x), StahlVal::BigNum(y))
        | (StahlVal::BigNum(y), StahlVal::BigRational(x)) => {
            (x.as_ref() * y.as_ref()).into_stahlval()
        }
        // Remaining cases that interact with `BigNum`. Probably not too common.
        (StahlVal::BigNum(x), StahlVal::BigNum(y)) => {
            let mut res = x.as_ref().clone();
            res += y.as_ref();
            res.into_stahlval()
        }
        (StahlVal::BigNum(x), StahlVal::IntV(y)) | (StahlVal::IntV(y), StahlVal::BigNum(x)) => {
            let mut res = x.as_ref().clone();
            res += *y;
            res.into_stahlval()
        }
        // Complex numbers
        (StahlVal::Complex(x), StahlVal::Complex(y)) => add_complex(x, y),
        (StahlVal::Complex(x), y) | (y, StahlVal::Complex(x)) => {
            debug_assert!(realp(y));
            add_complex(x, &StahlComplex::new(y.clone(), StahlVal::IntV(0)))
        }
        _ => unreachable!(),
    }
}

#[cold]
fn multiply_complex(x: &StahlComplex, y: &StahlComplex) -> Result<StahlVal> {
    // TODO: Optimize the implementation if needed.
    let real = add_two(
        &multiply_two(&x.re, &y.re)?,
        &negate(&multiply_two(&x.im, &y.im)?)?,
    )?;
    let im = add_two(&multiply_two(&x.re, &y.im)?, &multiply_two(&x.im, &y.re)?)?;
    StahlComplex::new(real, im).into_stahlval()
}

#[cold]
fn negate_complex(x: &StahlComplex) -> Result<StahlVal> {
    // TODO: Optimize the implementation if needed.
    StahlComplex::new(negate(&x.re)?, negate(&x.im)?).into_stahlval()
}

#[cold]
fn add_complex(x: &StahlComplex, y: &StahlComplex) -> Result<StahlVal> {
    // TODO: Optimize the implementation if needed.
    StahlComplex::new(add_two(&x.re, &y.re)?, add_two(&x.im, &y.im)?).into_stahlval()
}

#[cfg(test)]
mod num_op_tests {
    use super::*;
    use crate::{gc::Gc, rvals::StahlVal::*};
    use std::str::FromStr;

    #[test]
    fn division_test() {
        assert_eq!(
            divide_primitive(&[IntV(10), IntV(2)]).unwrap().to_string(),
            IntV(5).to_string()
        );
    }

    #[test]
    fn dvision_by_integer_zero_returns_positive_infinity() {
        // assert_eq!(
        //     divide_primitive(&[IntV(1), IntV(0)]).unwrap().to_string(),
        //     NumV(f64::INFINITY).to_string()
        // )

        assert!(divide_primitive(&[IntV(1), IntV(0)]).is_err())
    }

    #[test]
    fn division_on_single_integer_returns_reciprocal_rational() {
        assert_eq!(
            divide_primitive(&[IntV(10)]).unwrap().to_string(),
            Rational(Rational32::new(1, 10)).to_string()
        );
    }

    #[test]
    fn division_on_single_rational_returns_reciprocal_rational() {
        assert_eq!(
            divide_primitive(&[Rational32::new(2, 5).into_stahlval().unwrap()])
                .unwrap()
                .to_string(),
            Rational(Rational32::new(5, 2)).to_string()
        );
    }

    #[test]
    fn division_on_rational_with_numerator_one_returns_integer() {
        assert_eq!(
            divide_primitive(&[Rational32::new(1, 5).into_stahlval().unwrap()])
                .unwrap()
                .to_string(),
            IntV(5).to_string()
        );
    }

    #[test]
    fn division_on_bignum_returns_bigrational() {
        assert_eq!(
            divide_primitive(
                &([BigInt::from_str("18446744073709551616")
                    .unwrap()
                    .into_stahlval()
                    .unwrap(),])
            )
            .unwrap()
            .to_string(),
            BigRational(Gc::new(num::BigRational::new(
                BigInt::from(1),
                BigInt::from_str("18446744073709551616").unwrap()
            )))
            .to_string()
        );
    }

    #[test]
    fn multiplication_test() {
        let args = [IntV(10), IntV(2)];
        let got = multiply_primitive(&args).unwrap();
        let expected = IntV(20);
        assert_eq!(got, expected);
    }

    #[test]
    fn multiplication_different_types() {
        let args = [IntV(10), NumV(2.0)];
        let got = multiply_primitive(&args).unwrap();
        let expected = NumV(20.0);
        assert_eq!(got.to_string(), expected.to_string());
    }

    #[test]
    fn multiply_multiple_numbers() {
        assert_eq!(
            multiply_primitive(&[IntV(16), NumV(2.0), Rational(Rational32::new(1, 4))])
                .unwrap()
                .to_string(),
            NumV(8.0).to_string(),
        );
    }

    #[test]
    fn adding_exact_with_inexact_returns_inexact() {
        assert_eq!(
            add_primitive(&([IntV(10), NumV(2.0)])).unwrap().to_string(),
            NumV(12.0).to_string()
        );
        assert_eq!(
            add_primitive(
                &([
                    BigInt::from_str("18446744073709551616")
                        .unwrap()
                        .into_stahlval()
                        .unwrap(),
                    NumV(18446744073709551616.0),
                ])
            )
            .unwrap()
            .to_string(),
            NumV(18446744073709551616.0 * 2.0).to_string()
        );
        assert_eq!(
            add_primitive(
                &([
                    BigInt::from_str("18446744073709551616")
                        .unwrap()
                        .into_stahlval()
                        .unwrap(),
                    NumV(18446744073709551616.0),
                ])
            )
            .unwrap()
            .to_string(),
            NumV(18446744073709551616.0 * 2.0).to_string()
        );
        assert_eq!(
            add_primitive(&([Rational32::new(1, 2).into_stahlval().unwrap(), NumV(0.5),]))
                .unwrap()
                .to_string(),
            NumV(1.0).to_string()
        );
    }

    #[test]
    fn subtraction_different_types() {
        let args = [IntV(10), NumV(2.0)];
        let got = subtract_primitive(&args).unwrap();
        let expected = NumV(8.0);
        assert_eq!(got.to_string(), expected.to_string());
    }

    #[test]
    fn test_integer_add() {
        let args = [IntV(10), IntV(2)];
        let got = add_primitive(&args).unwrap();
        let expected = IntV(12);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_integer_sub() {
        let args = [IntV(10), IntV(2)];
        let got = subtract_primitive(&args).unwrap();
        let expected = IntV(8);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_exact_integer_sqrt() {
        assert_eq!(
            exact_integer_sqrt(&0.into()),
            (0.into_stahlval().unwrap(), 0.into_stahlval().unwrap()).into_stahlval()
        );
        assert_eq!(
            exact_integer_sqrt(&1.into()),
            (1.into_stahlval().unwrap(), 0.into_stahlval().unwrap()).into_stahlval()
        );
        assert_eq!(
            exact_integer_sqrt(&2.into()),
            (1.into_stahlval().unwrap(), 1.into_stahlval().unwrap()).into_stahlval()
        );
        assert_eq!(
            exact_integer_sqrt(&2.into()),
            (1.into_stahlval().unwrap(), 1.into_stahlval().unwrap()).into_stahlval()
        );
        assert_eq!(
            exact_integer_sqrt(&3.into()),
            (1.into_stahlval().unwrap(), 2.into_stahlval().unwrap()).into_stahlval()
        );
        assert_eq!(
            exact_integer_sqrt(&4.into()),
            (2.into_stahlval().unwrap(), 0.into_stahlval().unwrap()).into_stahlval()
        );
        assert_eq!(
            exact_integer_sqrt(&5.into()),
            (2.into_stahlval().unwrap(), 1.into_stahlval().unwrap()).into_stahlval()
        );
        assert_eq!(
            exact_integer_sqrt(&6.into()),
            (2.into_stahlval().unwrap(), 2.into_stahlval().unwrap()).into_stahlval()
        );
        assert_eq!(
            exact_integer_sqrt(&7.into()),
            (2.into_stahlval().unwrap(), 3.into_stahlval().unwrap()).into_stahlval()
        );
    }

    #[test]
    fn test_exact_integer_sqrt_fails_on_negative_or_noninteger() {
        assert!(exact_integer_sqrt(&(-7).into()).is_err());
        assert!(exact_integer_sqrt(&Rational32::new(-1, 2).into_stahlval().unwrap()).is_err());
        assert!(exact_integer_sqrt(
            &BigInt::from_str("-10000000000000000000000000000000000001")
                .unwrap()
                .into_stahlval()
                .unwrap()
        )
        .is_err());
        assert!(exact_integer_sqrt(
            &num::BigRational::new(
                BigInt::from_str("-10000000000000000000000000000000000001").unwrap(),
                BigInt::from_str("2").unwrap()
            )
            .into_stahlval()
            .unwrap()
        )
        .is_err());
        assert!(exact_integer_sqrt(&(1.0).into()).is_err());
        assert!(exact_integer_sqrt(
            &StahlComplex::new(1.into(), 1.into())
                .into_stahlval()
                .unwrap()
        )
        .is_err());
    }

    #[test]
    fn test_sqrt() {
        assert_eq!(sqrt(&4isize.into()).unwrap(), 2isize.into());
        assert_eq!(
            sqrt(
                &StahlComplex::new(0.into(), 2.into())
                    .into_stahlval()
                    .unwrap()
            )
            .unwrap(),
            StahlComplex::new(1.into(), 1.into())
                .into_stahlval()
                .unwrap()
        );
        assert_eq!(
            sqrt(
                &StahlComplex::new((-3).into(), (-4).into())
                    .into_stahlval()
                    .unwrap()
            )
            .unwrap(),
            StahlComplex::new(1.into(), (-2).into())
                .into_stahlval()
                .unwrap()
        );
    }
}
