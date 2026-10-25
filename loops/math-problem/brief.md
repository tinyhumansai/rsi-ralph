# Compute pi to 12 digits

`pi.estimate()` returns an approximation of pi. The score is the number of
correct decimal digits (higher is better, 15 is the most a double holds). The
estimate must stay fast: the scorer has a short timeout, so raising the loop
count of a slow series eventually fails.

## Where we are

{{incumbent}}

## What has been tried

{{history}}

## Your task

Pick one method that converges faster than the incumbent (for example a
machin-like arctangent formula) and have the implementer apply it, explaining
the method in the docstring.
