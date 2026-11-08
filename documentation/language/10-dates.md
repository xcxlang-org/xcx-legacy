# 10. Date and Time Type

The `date` type is an immutable date object with range validation (years 1–9999), automatic leap year handling, formatting and day arithmetic.

## 10.1. Creating Dates

ISO format (default):

```xcx
let (date) christmas = date("2024-12-25");
let (date) new_year = date("2025-01-01");
```

Custom format:

```xcx
let (date) european = date("25/12/2024", "DD/MM/YYYY");
let (date) american = date("12/25/2024", "MM/DD/YYYY");
```

Current date:

```xcx
let (date) today = date.now();
```

Invalid dates stop the program:

```xcx
# let (date) invalid = date("2024-13-45");  # HALT: invalid date
```

## 10.2. Properties

Read-only:

```xcx
let (int) year = christmas.year;    # 2024
let (int) month = christmas.month;  # 12
let (int) day = christmas.day;      # 25
```

## 10.3. Formatting

```xcx
>! christmas.format();              # "2024-12-25" (default)
>! christmas.format("DD-MM-YYYY");  # "25-12-2024"
>! christmas.format("MM/DD/YYYY");  # "12/25/2024"
```

Supported format tokens:

| Token | Meaning |
|-------|---------|
| `YYYY` | 4-digit year |
| `MM` | 2-digit month |
| `DD` | 2-digit day |
| `M` | 1-2 digit month |
| `D` | 1-2 digit day |

## 10.4. Arithmetic

Adding and subtracting days:

```xcx
let (date) next_week = today + 7;
let (date) last_week = today - 7;
```

Difference between dates (in days):

```xcx
let (int) days_to_christmas = christmas - today;
```

## 10.5. Comparisons

```xcx
let (boolean) is_before = (christmas < today);
let (boolean) is_equal = (christmas == date("2024-12-25"));  # true
```

Typical usage:

```xcx
if (today >= christmas) then;
    >! "Christmas has passed";
els;
    let (int) days_remaining = christmas - today;
    >! "Days until Christmas: " + days_remaining;
end;
```
