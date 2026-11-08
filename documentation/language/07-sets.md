# 7. Mathematical Sets

## 7.1. Rules

- Sets must be **homogeneous** (all elements of one type)
- No implicit type conversions
- Operations between different set types stop the program (HALT)
- `random.choice` on an empty set stops the program (HALT)

## 7.2. Declaration and Creation

Declaration syntax:

```xcx
let (set) name { TYPE = values };
```

### Numeric sets (ranges)

Range syntax is `start,,end` with an optional third `,,step`:

```xcx
let (set) even_numbers { N = 2,,20,,2 };    # 2,4,6,8,10,12,14,16,18,20
let (set) integers { Z = -5,,5 };           # -5,-4,-3,-2,-1,0,1,2,3,4,5
let (set) fractions { Q = 0.5,,2.5,,0.5 };  # 0.5,1.0,1.5,2.0,2.5
```

`N` = natural numbers, `Z` = integers, `Q` = fractions/rationals.

### Enumerated sets

```xcx
let (set) colors { S = "red", "green", "blue" };   # strings
let (set) booleans { B = true, false };            # booleans
let (set) letters { C = 'a', 'b', 'c', 'd' };      # characters
```

### Multiple ranges in one set

```xcx
let (set) mixed_ranges { N = 1,,5, 10,,15, 20,,25 };
```

## 7.3. Algebraic Operations

```xcx
let (set) A { N = 1,,10 };     # {1,2,3,4,5,6,7,8,9,10}
let (set) B { N = 5,,15 };     # {5,6,7,8,9,10,11,12,13,14,15}

# UNION - all elements from both sets:
let (set) union_set = A UNION B;
# {1,2,3,4,5,6,7,8,9,10,11,12,13,14,15}

# INTERSECTION - only common elements:
let (set) intersection_set = A INTERSECTION B;
# {5,6,7,8,9,10}

# DIFFERENCE - elements from the first set not present in the second:
let (set) diff_AB = A DIFFERENCE B;   # {1,2,3,4}
let (set) diff_BA = B DIFFERENCE A;   # {11,12,13,14,15}

# SYMMETRIC_DIFFERENCE - elements in exactly one of the sets:
let (set) sym_diff = A SYMMETRIC_DIFFERENCE B;
# {1,2,3,4,11,12,13,14,15}
```

Overlapping ranges are merged automatically:

```xcx
let (set) range1 { N = 1,,5 };
let (set) range2 { N = 4,,8 };
let (set) merged = range1 UNION range2;   # {1,2,3,4,5,6,7,8}
```

## 7.4. Random Element Selection

```xcx
let (int) random_even = random.choice from even_numbers;
let (str) random_color = random.choice from colors;
let (boolean) random_bool = random.choice from booleans;
```

`random.choice` works only with an **existing variable of type set**. Passing a set definition inline is an error:

```xcx
# let (int) x = random.choice from (N = 1,,100);  # ERROR
```

The result is typed to match the set elements:

```xcx
let (set) strings { S = "alice", "bob", "charlie" };
let (str) chosen = random.choice from strings;  # typed as str
```

## 7.5. Useful Methods

```xcx
let (boolean) is_empty = union_set.is_empty();  # false
```
