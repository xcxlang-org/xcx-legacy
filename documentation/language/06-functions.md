# 6. Functions and Procedures

## 6.1. Declaration and Definition

Simple function without parameters:

```xcx
dec func (greeting) {
    >! "Welcome to XCX!";
};

# Call:
greeting;
```

## 6.2. Parameters and Return Values

Function with parameters uses the `::` separator in both declaration and call:

```xcx
dec func (add :: let (int) a, let (int) b -> int) {
    return a + b;
};

let (int) result = add :: 5, 3;  # result = 8
```

Function with a boolean return value:

```xcx
dec func (is_even :: let (int) number -> boolean) {
    return (number % 2) == 0;
};

>! is_even :: 4;  # true
>! is_even :: 5;  # false
```

Function without a return type (a procedure):

```xcx
dec func (display_menu :: let (str) title) {
    >! "=== " + title + " ===";
    >! "1. First option";
    >! "2. Second option";
};
```

## 6.3. Recursion

```xcx
dec func (factorial :: let (int) n -> int) {
    if (n <= 1) then;
        return 1;
    els;
        return n * factorial :: (n - 1);
    end;
};

>! "5! = " + factorial :: 5;  # 120
```

## 6.4. Local Variables Inside Functions

```xcx
dec func (range_sum :: let (int) start, let (int) end -> int) {
    let (int) sum = 0;
    let (int) i = start;

    while (i <= end) do;
        sum = sum + i;
        i = i + 1;
    end;

    return sum;
};

>! range_sum :: 1, 10;  # 55
```
