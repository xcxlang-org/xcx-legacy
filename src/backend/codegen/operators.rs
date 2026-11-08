use super::{key_letter_of, val_description, Expectation, FunctionCodegen, Val};
use super::super::methods;
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::BinaryOp;
use cranelift::codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift::codegen::ir::{BlockArg, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, Value, types};

impl<'a> FunctionCodegen<'a> {
    pub(super) fn binary(
        &mut self,
        op: BinaryOp,
        left: &crate::frontend::ast::Expression,
        right: &crate::frontend::ast::Expression,
        location: crate::diagnostics::Location,
        expectation: Expectation,
    ) -> Result<Val, Diagnostic> {
        match op {
            BinaryOp::And | BinaryOp::Or => return self.logical(op, left, right, location),
            BinaryOp::From => return self.random_choice(left, right, location, expectation),
            BinaryOp::Has => return self.has_operator(left, right, location),
            BinaryOp::Union
            | BinaryOp::Intersection
            | BinaryOp::Difference
            | BinaryOp::SymmetricDifference => {
                let left = self.expression(left)?;
                let right = self.expression(right)?;
                let (Val::Set(a), Val::Set(b)) = (&left, &right) else {
                    return Err(self.binary_error(op, &left, &right, location));
                };
                let name = match op {
                    BinaryOp::Union => "xcx_set_union",
                    BinaryOp::Intersection => "xcx_set_intersection",
                    BinaryOp::Difference => "xcx_set_difference",
                    _ => "xcx_set_symmetric_difference",
                };
                let call = self.builder.ins().call(self.helpers.reference(name), &[*a, *b]);
                return Ok(Val::Set(self.builder.inst_results(call)[0]));
            }
            _ => {}
        }

        let left_expectation = match (op, expectation) {
            (BinaryOp::Add, Expectation::Str) => Expectation::Str,
            (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul, Expectation::Int) => Expectation::Int,
            (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul, Expectation::Float) => Expectation::Float,
            _ => Expectation::Any,
        };
        let left = self.expression_expect(left, left_expectation)?;
        let right_expectation = match (&left, op, expectation) {
            (_, BinaryOp::Add, _) if matches!(left, Val::Str { .. } | Val::Bool(_) | Val::Char(_)) => {
                Expectation::Str
            }
            (_, BinaryOp::Add, Expectation::Str)
                if matches!(left, Val::Int(_) | Val::Float(_)) =>
            {
                Expectation::Str
            }
            (_, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul, Expectation::Int)
                if matches!(left, Val::Int(_)) =>
            {
                Expectation::Int
            }
            (_, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul, Expectation::Float)
                if matches!(left, Val::Float(_)) =>
            {
                Expectation::Float
            }
            _ => Expectation::Any,
        };
        let right = self.expression_expect(right, right_expectation)?;

        match op {
            BinaryOp::Div => {
                let l = self.to_float(&left)?;
                let r = self.to_float(&right)?;
                Ok(Val::Float(self.builder.ins().fdiv(l, r)))
            }
            BinaryOp::Add | BinaryOp::Sub if matches!(left, Val::Date(_)) => {
                match (&left, &right, op) {
                    (Val::Date(a), Val::Int(b), BinaryOp::Add) => {
                        Ok(Val::Date(self.builder.ins().iadd(*a, *b)))
                    }
                    (Val::Date(a), Val::Int(b), BinaryOp::Sub) => {
                        Ok(Val::Date(self.builder.ins().isub(*a, *b)))
                    }
                    (Val::Date(a), Val::Date(b), BinaryOp::Sub) => {
                        Ok(Val::Int(self.builder.ins().isub(*a, *b)))
                    }
                    _ => Err(self.binary_error(op, &left, &right, location)),
                }
            }
            BinaryOp::Mod => match (&left, &right) {
                (Val::Int(a), Val::Int(b)) => Ok(Val::Int(self.builder.ins().srem(*a, *b))),
                _ => Err(self.binary_error(op, &left, &right, location)),
            },
            BinaryOp::Concat => match (&left, &right) {
                (Val::Int(a), Val::Int(b)) => {
                    let call = self.builder.ins().call(self.helpers.reference("xcx_concat_ii"), &[*a, *b]);
                    Ok(Val::Int(self.builder.inst_results(call)[0]))
                }
                _ => self.string_concat(&left, &right),
            },
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                let string_operation = op == BinaryOp::Add
                    && (matches!(left, Val::Str { .. })
                        || matches!(right, Val::Str { .. })
                        || matches!(left, Val::Bool(_) | Val::Char(_))
                        || matches!(right, Val::Bool(_) | Val::Char(_)));
                if string_operation {
                    return self.string_concat(&left, &right);
                }
                let floating = matches!(left, Val::Float(_)) || matches!(right, Val::Float(_));
                if floating {
                    let l = self.to_float(&left)?;
                    let r = self.to_float(&right)?;
                    Ok(Val::Float(match op {
                        BinaryOp::Add => self.builder.ins().fadd(l, r),
                        BinaryOp::Sub => self.builder.ins().fsub(l, r),
                        BinaryOp::Mul => self.builder.ins().fmul(l, r),
                        _ => unreachable!("handled above"),
                    }))
                } else {
                    let (Val::Int(a), Val::Int(b)) = (&left, &right) else {
                        return Err(self.binary_error(op, &left, &right, location));
                    };
                    Ok(Val::Int(match op {
                        BinaryOp::Add => self.builder.ins().iadd(*a, *b),
                        BinaryOp::Sub => self.builder.ins().isub(*a, *b),
                        BinaryOp::Mul => self.builder.ins().imul(*a, *b),
                        _ => unreachable!("handled above"),
                    }))
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le
            | BinaryOp::Ge => self.comparison(op, &left, &right, location),
            BinaryOp::And | BinaryOp::Or | BinaryOp::From | BinaryOp::Has
            | BinaryOp::Union | BinaryOp::Intersection | BinaryOp::Difference
            | BinaryOp::SymmetricDifference => unreachable!("handled above"),
        }
    }

    fn random_choice(
        &mut self,
        marker: &crate::frontend::ast::Expression,
        set: &crate::frontend::ast::Expression,
        location: crate::diagnostics::Location,
        expectation: Expectation,
    ) -> Result<Val, Diagnostic> {
        let _ = marker;
        let Val::Set(ptr) = self.expression(set)? else {
            return Err(Diagnostic::new(
                "`random.choice from` requires a set",
                location,
            ));
        };
        match expectation {
            Expectation::Int => {
                let call = self.builder.ins().call(self.helpers.reference("xcx_choice_int"), &[ptr]);
                Ok(Val::Int(self.builder.inst_results(call)[0]))
            }
            Expectation::Float => {
                let call = self.builder.ins().call(self.helpers.reference("xcx_choice_float"), &[ptr]);
                Ok(Val::Float(self.builder.inst_results(call)[0]))
            }
            Expectation::Bool => {
                let call = self.builder.ins().call(self.helpers.reference("xcx_choice_bool"), &[ptr]);
                Ok(Val::Bool(self.builder.inst_results(call)[0]))
            }
            Expectation::Char => {
                let call = self.builder.ins().call(self.helpers.reference("xcx_choice_char"), &[ptr]);
                Ok(Val::Char(self.builder.inst_results(call)[0]))
            }
            Expectation::Str => {
                let slot = self
                    .builder
                    .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
                let slot_address = self.builder.ins().stack_addr(types::I64, slot, 0);
                let call = self.builder.ins().call(
                    self.helpers.reference("xcx_choice_str"),
                    &[ptr, slot_address],
                );
                let ptr = self.builder.inst_results(call)[0];
                let len = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), slot_address, 0);
                Ok(Val::Str { ptr, len })
            }
            Expectation::Any => Err(Diagnostic::new(
                "cannot determine the element type of `random.choice` — assign it to a typed variable",
                location,
            )),
            Expectation::Date | Expectation::Json => Err(Diagnostic::new(
                "internal: `random.choice` cannot yield date or json elements",
                location,
            )),
        }
    }

    fn has_operator(
        &mut self,
        left: &crate::frontend::ast::Expression,
        right: &crate::frontend::ast::Expression,
        location: crate::diagnostics::Location,
    ) -> Result<Val, Diagnostic> {
        let left = self.expression(left)?;
        match &left {
            Val::Str { .. } => {
                let right = self.expression_expect(right, Expectation::Str)?;
                let (Val::Str { ptr: hp, len: hl }, Val::Str { ptr: np, len: nl }) = (&left, &right)
                else {
                    return Err(Diagnostic::new("`has` on str requires str", location));
                };
                Ok(Val::Bool(self.call0(
                    self.helpers.reference("xcx_str_contains"),
                    &[*hp, *hl, *np, *nl],
                )))
            }
            Val::Set(set) => {
                let argument = self.expression(right)?;
                methods::set_has(self, *set, argument, location)
            }
            Val::Map(map, _) => {
                let key = self.expression(right)?;
                let (key_a, key_b) = match &key {
                    Val::Int(v) => (*v, self.builder.ins().iconst(types::I64, 0)),
                    Val::Str { ptr, len } => (*ptr, *len),
                    _ => {
                        return Err(Diagnostic::new(
                            "map keys must be int or str",
                            location,
                        ));
                    }
                };
                let name = format!("xcx_map_{}_has", key_letter_of(&key));
                Ok(Val::Bool(self.call0(
                    self.helpers.reference(&name),
                    &[*map, key_a, key_b],
                )))
            }
            other => Err(Diagnostic::new(
                format!(
                    "`has` works on str, set and map, got {}",
                    val_description(other)
                ),
                location,
            )),
        }
    }

    fn logical(
        &mut self,
        op: BinaryOp,
        left: &crate::frontend::ast::Expression,
        right: &crate::frontend::ast::Expression,
        location: crate::diagnostics::Location,
    ) -> Result<Val, Diagnostic> {
        let left_value = self.bool_expression(left)?;
        let eval_right = self.builder.create_block();
        let merge = self.builder.create_block();
        self.builder.append_block_param(merge, types::I8);
        let short_circuit_value = [BlockArg::from(left_value)];
        match op {
            BinaryOp::And => {
                self.builder
                    .ins()
                    .brif(left_value, eval_right, &[], merge, &short_circuit_value);
            }
            BinaryOp::Or => {
                self.builder
                    .ins()
                    .brif(left_value, merge, &short_circuit_value, eval_right, &[]);
            }
            _ => unreachable!("logical called with a non-logical operator"),
        }
        self.builder.switch_to_block(eval_right);
        self.builder.seal_block(eval_right);
        let right_value = self.bool_expression(right)?;
        let evaluated_value = [BlockArg::from(right_value)];
        self.builder.ins().jump(merge, &evaluated_value);
        self.builder.switch_to_block(merge);
        self.builder.seal_block(merge);
        let result = self.builder.block_params(merge)[0];
        let _ = location;
        Ok(Val::Bool(result))
    }

    fn comparison(
        &mut self,
        op: BinaryOp,
        left: &Val,
        right: &Val,
        location: crate::diagnostics::Location,
    ) -> Result<Val, Diagnostic> {
        let condition = match (left, right) {
            (Val::Int(a), Val::Int(b)) => self.emit_int_condition(op, *a, *b),
            (Val::Bool(a), Val::Bool(b)) => self.emit_int_condition(op, *a, *b),
            (Val::Char(a), Val::Char(b)) => self.emit_int_condition(op, *a, *b),
            (Val::Float(a), Val::Float(b)) => self.emit_float_condition(op, *a, *b),
            (Val::Float(a), Val::Int(b)) => {
                let promoted = self.builder.ins().fcvt_from_sint(types::F64, *b);
                self.emit_float_condition(op, *a, promoted)
            }
            (Val::Int(a), Val::Float(b)) => {
                let promoted = self.builder.ins().fcvt_from_sint(types::F64, *a);
                self.emit_float_condition(op, promoted, *b)
            }
            (Val::Date(a), Val::Date(b)) => self.emit_int_condition(op, *a, *b),
            (Val::Str { ptr: a_ptr, len: a_len }, Val::Str { ptr: b_ptr, len: b_len }) => {
                let ordering = self.call0(
                    self.helpers.reference("xcx_str_compare"),
                    &[*a_ptr, *a_len, *b_ptr, *b_len],
                );
                let zero = self.builder.ins().iconst(types::I64, 0);
                let condition = match op {
                    BinaryOp::Eq => IntCC::Equal,
                    BinaryOp::NotEq => IntCC::NotEqual,
                    BinaryOp::Lt => IntCC::SignedLessThan,
                    BinaryOp::Le => IntCC::SignedLessThanOrEqual,
                    BinaryOp::Gt => IntCC::SignedGreaterThan,
                    BinaryOp::Ge => IntCC::SignedGreaterThanOrEqual,
                    _ => unreachable!("comparison called with a non-comparison operator"),
                };
                self.builder.ins().icmp(condition, ordering, zero)
            }
            _ => return Err(self.binary_error(op, left, right, location)),
        };
        Ok(Val::Bool(condition))
    }

    fn emit_int_condition(&mut self, op: BinaryOp, a: Value, b: Value) -> Value {
        let condition = match op {
            BinaryOp::Eq => IntCC::Equal,
            BinaryOp::NotEq => IntCC::NotEqual,
            BinaryOp::Lt => IntCC::SignedLessThan,
            BinaryOp::Le => IntCC::SignedLessThanOrEqual,
            BinaryOp::Gt => IntCC::SignedGreaterThan,
            BinaryOp::Ge => IntCC::SignedGreaterThanOrEqual,
            _ => unreachable!("comparison called with a non-comparison operator"),
        };
        self.builder.ins().icmp(condition, a, b)
    }

    fn emit_float_condition(&mut self, op: BinaryOp, a: Value, b: Value) -> Value {
        let condition = match op {
            BinaryOp::Eq => FloatCC::Equal,
            BinaryOp::NotEq => FloatCC::NotEqual,
            BinaryOp::Lt => FloatCC::LessThan,
            BinaryOp::Le => FloatCC::LessThanOrEqual,
            BinaryOp::Gt => FloatCC::GreaterThan,
            BinaryOp::Ge => FloatCC::GreaterThanOrEqual,
            _ => unreachable!("comparison called with a non-comparison operator"),
        };
        self.builder.ins().fcmp(condition, a, b)
    }

    fn string_concat(&mut self, left: &Val, right: &Val) -> Result<Val, Diagnostic> {
        let slot = self
            .builder
            .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
        let slot_address = self.builder.ins().stack_addr(types::I64, slot, 0);
        let (function, arguments) = match (left, right) {
            (Val::Str { ptr, len }, Val::Str { ptr: p2, len: l2 }) => {
                ("xcx_concat_ss", vec![*ptr, *len, *p2, *l2, slot_address])
            }
            (Val::Str { ptr, len }, Val::Int(v)) => {
                ("xcx_concat_si", vec![*ptr, *len, *v, slot_address])
            }
            (Val::Int(v), Val::Str { ptr, len }) => {
                ("xcx_concat_is", vec![*v, *ptr, *len, slot_address])
            }
            (Val::Str { ptr, len }, Val::Float(v)) => {
                ("xcx_concat_sf", vec![*ptr, *len, *v, slot_address])
            }
            (Val::Float(v), Val::Str { ptr, len }) => {
                ("xcx_concat_fs", vec![*v, *ptr, *len, slot_address])
            }
            (Val::Str { ptr, len }, Val::Bool(v)) => {
                ("xcx_concat_sb", vec![*ptr, *len, *v, slot_address])
            }
            (Val::Bool(v), Val::Str { ptr, len }) => {
                ("xcx_concat_bs", vec![*v, *ptr, *len, slot_address])
            }
            (Val::Str { ptr, len }, Val::Char(v)) => {
                ("xcx_concat_sc" , vec![*ptr, *len, *v, slot_address])
            }
            (Val::Char(v), Val::Str { ptr, len }) => {
                ("xcx_concat_cs", vec![*v, *ptr, *len, slot_address])
            }
            _ => {
                return Err(Diagnostic::new(
                    "`+` with these operand types is not generated",
                    crate::diagnostics::Location { line: 0, column: 0 },
                ));
            }
        };
        let call = self.builder.ins().call(self.helpers.reference(function), &arguments);
        let out_ptr = self.builder.inst_results(call)[0];
        let out_len = self
            .builder
            .ins()
            .load(types::I64, MemFlagsData::new(), slot_address, 0);
        Ok(Val::Str { ptr: out_ptr, len: out_len })
    }

    fn to_float(&mut self, value: &Val) -> Result<Value, Diagnostic> {
        Ok(match value {
            Val::Float(v) => *v,
            Val::Int(v) => self.builder.ins().fcvt_from_sint(types::F64, *v),
            other => {
                return Err(self.expression_error(other, "floating-point arithmetic"));
            }
        })
    }
}
