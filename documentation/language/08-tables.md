# 8. Structural Tables

Tables are data structures with typed columns and rows, automatic ID generation and built-in filtering methods.

## 8.1. Declaration

```xcx
let (table) employees {
    columns = [
        id :: int @auto,           # Automatic ID
        first_name :: str,         # Text column
        last_name :: str,          # Text column
        age :: int,                # Integer column
        salary :: float,           # Floating point column
        active :: boolean,         # Boolean column
        hire_date :: date          # Date column
    ]
    rows = [ EMPTY ]               # Initially empty
};
```

The `@auto` column fills itself automatically on `insert` — you skip it when inserting data.

## 8.2. CRUD Operations

```xcx
let (table) products {
    columns = [
        id :: int @auto,
        name :: str,
        price :: float
    ]
    rows = [ EMPTY ]
};

# CREATE - adding records:
products.insert("Laptop", 2999.99);
products.insert("Phone", 1499.50);
products.insert("Mouse", 79.99);

# READ - accessing fields by index:
>! "First product: " + products[0].name;
>! "Price: " + products[0].price;

# UPDATE - replacing a record's data:
products.update(1, ["Smartphone", 1599.99]);

# DELETE - removing a record by index:
products.delete(2);
```

## 8.3. Filtering with `.where()`

`.where()` returns a new table with matching rows:

```xcx
let (table) expensive_products = products.where(price > 1000);
let (table) cheap_products = products.where(price < 100);
```

Complex conditions combine with `AND` / `OR`:

```xcx
let (table) active_employees = employees.where(
    active == true AND age < 65
);
```

Filtering by date:

```xcx
let (table) new_employees = employees.where(
    hire_date >= date("2024-01-01")
);
```

Chained filtering:

```xcx
let (table) result = employees
    .where(age > 25)
    .where(salary > 5000)
    .where(active == true);
```

Inside `.where()`, column names refer to the table's columns — they don't conflict with variables outside.

## 8.4. Indexing and Counting

```xcx
>! "Third product: " + products[2].name;
>! "Number of products: " + products.count();

# Iterating over a table:
for i in 0 to (products.count() - 1);
    >! "Product " + i + ": " + products[i].name;
end;
```

## 8.5. Joining Tables (`.join()`)

`.join()` performs an INNER JOIN of two tables based on matching values in key columns:

```xcx
let (table) result = table_A.join(table_B, "key_A", "key_B");
```

Example:

```xcx
let (table) users {
    columns = [ id :: int @auto, name :: str ]
    rows = [ EMPTY ]
};
users.insert("Adam");
users.insert("Eve");

let (table) orders {
    columns = [ user_id :: int, product :: str ]
    rows = [ EMPTY ]
};
orders.insert(1, "Laptop");
orders.insert(2, "Phone");
orders.insert(1, "Mouse");

let (table) report = users.join(orders, "id", "user_id");
```

Column name conflicts are resolved automatically:

- Columns with the same name in both tables get aliases in the format `{table_name}_{column_name}`
- The join key column keeps its original name
- Columns without conflicts keep their original names

Example: table A has `id, name, department`, table B has `id, name, budget` — the join result has columns `id, A_name, department, B_name, budget`.

## 8.6. Lambda Join

For custom join conditions, pass a lambda expression instead of key names:

```xcx
let (table) result = table_A.join(table_B, lambda_expression);
```

Inside the lambda these row variables are available:

- `user` — row from the left table
- `order` — row from the right table
- Column access: `user.id`, `order.price`
