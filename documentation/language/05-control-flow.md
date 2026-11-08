# 5. Control Structures

## 5.1. Conditional Statements

Simple `if` / `els`:

```xcx
let (int) grade = 85;

if (grade >= 60) then;
    >! "Passed";
els;
    >! "Failed";
end;
```

Multiple conditions with `if` / `elf` / `els`:

```xcx
let (int) points = 75;

if (points >= 90) then;
    >! "Grade: A";
elf (points >= 75) then;
    >! "Grade: B";
elf (points >= 60) then;
    >! "Grade: C";
els;
    >! "Grade: F";
end;
```

Nested conditions:

```xcx
let (boolean) logged_in = true;
let (int) access_level = 3;

if (logged_in) then;
    if (access_level >= 2) then;
        >! "Full access";
    els;
        >! "Limited access";
    end;
els;
    >! "No access";
end;
```

## 5.2. Loops

`for` loop over a range of numbers:

```xcx
for i in 1 to 5;
    >! "Number: " + i;
end;
```

`while` loop:

```xcx
let (int) counter = 0;

while (counter < 3) do;
    >! "Iteration: " + counter;
    counter = counter + 1;
end;
```

Infinite loop with a break condition:

```xcx
let (int) attempt = 1;

while (true) do;
    >! "Attempt: " + attempt;

    if (attempt >= 3) then;
        break;  # Exit the loop
    end;

    attempt = attempt + 1;
end;
```

## 5.3. Flow Control

`continue` skips the rest of the current iteration:

```xcx
for i in 1 to 10;
    if (i % 2 == 0) then;
        continue;  # Skip even numbers
    end;
    >! "Odd: " + i;
end;
```
