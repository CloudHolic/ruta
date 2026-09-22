//! The counted `for`: set up once, stepped each time round.

use crate::value::Value;

use super::error::Error;
use super::state::Vm;

/// Answers whether the body runs at all.
pub(super) fn prepare(vm: &mut Vm, control: u32) -> Result<bool, Error> {
    let init = vm.stack.at(control);
    let limit = vm.stack.at(control + 1);
    let step = vm.stack.at(control + 2);

    if let (Value::Int(init), Value::Int(step)) = (init, step) {
        if step == 0 {
            return Err(vm.throw("'for' step is zero"));
        }

        let Some(limit) = integer_limit(vm, limit, step)? else {
            return Ok(false);
        };

        if (step > 0 && init > limit) || (step < 0 && init < limit) {
            return Ok(false);
        }

        let count = match step > 0 {
            true => (limit as u64).wrapping_sub(init as u64) / step as u64,
            false => (init as u64).wrapping_sub(limit as u64) / (step as u64).wrapping_neg(),
        };

        vm.stack.put(control + 1, Value::Int(count as i64));
        vm.stack.put(control + 3, Value::Int(init));

        return Ok(true);
    }

    let limit = float(vm, limit, "limit")?;
    let step = float(vm, step, "step")?;
    let init = float(vm, init, "initial value")?;

    if step == 0.0 {
        return Err(vm.throw("'for' step is zero"));
    }

    let runs = match step > 0.0 {
        true => init <= limit,
        false => init >= limit,
    };

    if runs {
        vm.stack.put(control, Value::Float(init));
        vm.stack.put(control + 1, Value::Float(limit));
        vm.stack.put(control + 2, Value::Float(step));
        vm.stack.put(control + 3, Value::Float(init));
    }

    Ok(runs)
}

/// Answers whether the body runs again.
pub(super) fn advance(vm: &mut Vm, control: u32) -> bool {
    match (
        vm.stack.at(control),
        vm.stack.at(control + 1),
        vm.stack.at(control + 2),
    ) {
        (Value::Int(index), Value::Int(left), Value::Int(step)) => {
            if left == 0 {
                return false;
            }

            let next = index.wrapping_add(step);

            vm.stack.put(control, Value::Int(next));
            vm.stack.put(control + 1, Value::Int(left.wrapping_sub(1)));
            vm.stack.put(control + 3, Value::Int(next));

            true
        }
        (Value::Float(index), Value::Float(limit), Value::Float(step)) => {
            let next = index + step;
            let runs = match step > 0.0 {
                true => next <= limit,
                false => limit <= next,
            };

            if runs {
                vm.stack.put(control, Value::Float(next));
                vm.stack.put(control + 3, Value::Float(next));
            }

            runs
        }
        other => unreachable!("a loop that was never prepared: {other:?}"),
    }
}

/// An integer loop's limit. A float is rounded otward the start, and one past either end of the range
/// means the loop runs to that end or not at all.
fn integer_limit(vm: &mut Vm, limit: Value, step: i64) -> Result<Option<i64>, Error> {
    let number = match vm.to_number(limit) {
        Some(Value::Int(number)) => return Ok(Some(number)),
        Some(Value::Float(number)) => number,
        _ => return Err(bad(vm, limit, "limit")),
    };

    let rounded = match step < 0 {
        true => number.ceil(),
        false => number.floor(),
    };

    if rounded >= -(2.0_f64.powi(63)) && rounded < 2.0_f64.powi(63) {
        return Ok(Some(rounded as i64));
    }

    Ok(match (0.0 < number, step < 0) {
        (true, true) | (false, false) => None,
        (true, false) => Some(i64::MAX),
        (false, true) => Some(i64::MIN),
    })
}

fn float(vm: &mut Vm, value: Value, what: &str) -> Result<f64, Error> {
    match vm.to_number(value) {
        Some(Value::Int(number)) => Ok(number as f64),
        Some(Value::Float(number)) => Ok(number),
        _ => Err(bad(vm, value, what)),
    }
}

fn bad(vm: &mut Vm, value: Value, what: &str) -> Error {
    vm.throw(format!(
        "bad 'for' {what} (number expected, got {})",
        value.type_name()
    ))
}
