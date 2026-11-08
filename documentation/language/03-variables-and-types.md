# 3. Data Types and Variables

## 3.1. Primitive Types

### Integer numbers (`int`)

```xcx
let (int) age = 25;
let (int) temperature = -10;
let (int) maximum = 999999;
```

### Strings (`str`)

```xcx
let (str) name = "Anna";
let (str) description = "This is a long description";
let (str) quote = "This is a \"quote\" in the text";
```

### Boolean values (`boolean`)

```xcx
let (boolean) active = true;
let (boolean) verified = false;
```

### Floating point numbers (`float`)

```xcx
let (float) price = 19.99;
let (float) exchange_rate = 3.8456;
let (float) pi = 3.14159;
```

## 3.2. Variables and Constants

Variables can be modified:

```xcx
let (int) counter = 0;
counter = counter + 1;        # Valid
```

Constants are immutable after initialization:

```xcx
const (str) VERSION = "1.0.0";
# VERSION = "2.0.0";           # ERROR: Cannot change a constant
```

## 3.3. Declaration and Initialization

Full declaration with type:

```xcx
let (int) number = 42;
```

Declaration without initial value:

```xcx
let (str) text;
text = "Hello";
```

Multiple variables in one line:

```xcx
let (int) x = 1, y = 2, z = 3;
```

## 3.4. Scope of Visibility

Variables are visible within their own context — a variable declared inside a function is not visible outside:

```xcx
dec func (example) {
    let (int) local = 10;    # Visible only inside the function
};

# local = 20;                 # ERROR: Undefined variable
```
