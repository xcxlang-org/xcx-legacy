# 2. Language Basics

## 2.1. Basic Syntax

Every statement ends with a semicolon:

```xcx
let (int) x = 5;
```

Displaying data:

```xcx
>! "Hello world";           # Displays text
>! variable;                # Displays variable value
```

Reading data from the user:

```xcx
>? variable_name;           # Reads data from the user
```

## 2.2. Comments

Single-line comment:

```xcx
# this is a single-line comment
```

Multi-line comment:

```xcx
##
this is a
multi-line comment
##
```

## 2.3. Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Variables | `snake_case` | `let (int) user_count = 100;` |
| Functions | `snake_case` | `dec func (calculate_sum) { ... };` |
| Constants | `UPPER_SNAKE_CASE` | `const (int) MAXIMUM_LENGTH = 1000;` |
| Tables | plural noun | `let (table) users { ... };` |

```xcx
let (int) user_count = 100;
let (str) first_and_last_name = "John Smith";
const (int) MAXIMUM_LENGTH = 1000;
```
