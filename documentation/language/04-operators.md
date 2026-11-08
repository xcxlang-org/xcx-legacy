# 4. Operators and Expressions

## 4.1. Arithmetic Operators

```xcx
let (int) a = 10;
let (int) b = 3;

>! a + b;        # 13 - addition
>! a - b;        # 7 - subtraction
>! a * b;        # 30 - multiplication
>! a / b;        # 3.333... - division
>! a % b;        # 1 - modulo (remainder from division)
```

Operations on floating point numbers:

```xcx
let (float) x = 7.5;
let (float) y = 2.5;
>! x / y;        # 3.0
```

## 4.2. Integer Concatenation (`++`)

Joins digits without mathematical addition:

```xcx
let (int) prefix = 48;
let (int) number = 123456789;
let (int) phone = prefix ++ number;  # 48123456789
```

Useful for phone numbers, identification codes and serial numbers.

## 4.3. Comparison Operators

```xcx
let (int) x = 5;
let (int) y = 10;

>! x > y;         # false - greater than
>! x < y;         # true - less than
>! x >= y;        # false - greater than or equal
>! x <= y;        # true - less than or equal
>! x == y;        # false - equal
>! x != y;        # true - not equal
```

Strings compare lexicographically:

```xcx
let (str) a = "abc";
let (str) b = "def";
>! a == b;        # false
>! a < b;         # true
```

## 4.4. Logical Operators

```xcx
let (boolean) p = true;
let (boolean) q = false;

>! p AND q;       # false - conjunction
>! p OR q;        # true - disjunction
>! NOT p;         # false - negation
```

Compound conditions:

```xcx
let (int) age = 25;
let (boolean) has_permissions = true;

if (age >= 18 AND has_permissions) then;
    >! "Access granted";
end;
```

## 4.5. Containment Operator (`has`)

Checks whether a collection contains an element, or a text contains a substring:

```xcx
# Substring in text:
let (str) email = "user@xcx.com";
if (email has "@") then;
    >! "Valid email format";
end;

# Element in a set:
let (set) blocked_domains {
    S = "spam.com", "virus.com"
};
if (blocked_domains has "spam.com") then;
    >! "Domain blocked";
end;

let (set) numbers { N = 1,,10 };
if (numbers has 5) then;
    >! "Number 5 is in the set";
end;
```

## 4.6. Special Operators

| Operator | Meaning | Example |
|----------|---------|---------|
| `->` | return type of a function | `dec func (double :: let (int) x -> int)` |
| `::` | type/value separator | `let (map :: str <-> int) age = { "John" :: 25 };` |
| `<->` | key-value type relationship in maps | `let (map :: int <-> str) codes = { 200 :: "OK" };` |
