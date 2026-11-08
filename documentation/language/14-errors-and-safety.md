# 14. Error Handling and Safety

## 14.1. The HALT System

XCX stops the program immediately (HALT) when it detects:

- Operations on incompatible types
- Access to non-existent data (e.g. a missing map key)
- Security violations (e.g. path traversal in `store`)
- Invalid values (e.g. an impossible date)

Examples:

```xcx
# Different set types:
let (set) A { N = 1,,10 };
let (set) B { S = "a", "b" };
# A UNION B;                                  # HALT: different set types

# random.choice on an empty set:
# random.choice from empty_set;               # HALT: empty set

# Missing key in a map:
let (map :: str <-> int) map_obj = { "exists" :: 42 };
# map_obj.get("not_exists");                  # HALT: missing key

# Type mismatch:
let (int) number = 42;
# number = "text";                            # HALT: type mismatch
```

## 14.2. The `halt` Statement

You can stop the program deliberately after detecting an invalid state:

```xcx
dec func (calculate_tax :: let (float) income -> float) {
    if (income < 0) then;
        >! "Error: negative income";
        halt;  # Stop the program
    end;

    return income * 0.19;
};
```

## 14.3. Safety Rules

- **No null values** — `let (str) text = null;` is not allowed
- **Constants are immutable** — changing a `const` stops the program
- **Values are validated at creation** — e.g. `date("2024-13-45")` stops the program

## 14.4. Debugging

Practical strategies:

1. Add `>!` messages at key points in the code
2. Validate input data before processing it
3. Test edge cases
4. Use the interpreter's interactive mode

```xcx
dec func (process_data :: let (int) value -> int) {
    >! "Processing value: " + value;  # Debug message

    if (value <= 0) then;
        >! "ERROR: value must be positive";
        halt;
    end;

    let (int) result = value * 2;
    >! "Result: " + result;           # Debug message
    return result;
};
```
