# Bring slug.py up to the standard

`slug.slugify` works but breaks the team's coding standard. The score is the
number of violations (lower is better, 0 is clean):

1. the module has a docstring;
2. every function has a docstring;
3. every parameter and return value has a type annotation;
4. no bare `except:`;
5. no line is longer than 79 characters.

Behavior must not change: a gate runs `slugify` against fixed cases first, and
a candidate that fails it is rejected however tidy it looks. Unicode letters
are kept in slugs.

## Where we are

{{incumbent}}

## What has been tried

{{history}}

## Your task

Have the implementer bring `slug.py` to the standard in one change, keeping
every behavior the cases pin down.
