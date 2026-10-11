"""Original independent integer oracles for private Euler-inspired variants.

No external solution code is copied. Paired algorithms use different
representations and traversals; disagreement aborts benchmark generation.
"""
import itertools
import math


def assignment_dp(matrix):
    """Maximize an assignment with subset dynamic programming."""
    n = len(matrix)
    best = {0: 0}
    for row in matrix:
        following = {}
        for mask, score in best.items():
            for col in range(n):
                if not mask & (1 << col):
                    new = mask | (1 << col)
                    value = score + row[col]
                    following[new] = max(following.get(new, value), value)
        best = following
    return best[(1 << n) - 1]


def assignment_matching(matrix):
    """Solve the negative-cost assignment by primal-dual augmenting paths."""
    n = len(matrix)
    u, v, matching, previous = ([0] * (n + 1) for _ in range(4))
    for row in range(1, n + 1):
        matching[0] = row
        col = 0
        slack = [math.inf] * (n + 1)
        visited = [False] * (n + 1)
        while True:
            visited[col] = True
            active = matching[col]
            delta, following = math.inf, 0
            for other in range(1, n + 1):
                if not visited[other]:
                    cost = -matrix[active - 1][other - 1] - u[active] - v[other]
                    if cost < slack[other]:
                        slack[other], previous[other] = cost, col
                    if slack[other] < delta:
                        delta, following = slack[other], other
            for other in range(n + 1):
                if visited[other]:
                    u[matching[other]] += delta
                    v[other] -= delta
                else:
                    slack[other] -= delta
            col = following
            if matching[col] == 0:
                break
        while col:
            before = previous[col]
            matching[col] = matching[before]
            col = before
    return sum(matrix[matching[col] - 1][col - 1] for col in range(1, n + 1))


def is_prime(n):
    """Trial-division primality independent of the dense sieve oracle."""
    return n >= 2 and all(n % divisor for divisor in range(2, math.isqrt(n) + 1))


def digit_sum(n):
    """Sum decimal digits of a positive integer."""
    return sum(map(int, str(n)))


def harshad_tree(limit):
    """Grow right-truncatable Harshad prefixes and test strong prime extensions."""
    total = 0
    pending = list(range(1, 10))
    while pending:
        prefix = pending.pop()
        if prefix * 10 >= limit:
            continue
        if is_prime(prefix // digit_sum(prefix)):
            for digit in (1, 3, 7, 9):
                value = prefix * 10 + digit
                if value < limit and is_prime(value):
                    total += value
        for digit in range(10):
            value = prefix * 10 + digit
            if value < limit and value % digit_sum(value) == 0:
                pending.append(value)
    return total


def harshad_sieve(limit):
    """Enumerate primes first, then test the complete decimal prefix chain."""
    prime = bytearray(b'\1') * max(limit, 2)
    prime[:2] = b'\0\0'
    for factor in range(2, math.isqrt(max(limit - 1, 0)) + 1):
        if prime[factor]:
            for multiple in range(factor * factor, limit, factor):
                prime[multiple] = 0
    total = 0
    for value in range(10, limit):
        if not prime[value]:
            continue
        prefix = value // 10
        if prefix % digit_sum(prefix) or not prime[prefix // digit_sum(prefix)]:
            continue
        while prefix and prefix % digit_sum(prefix) == 0:
            prefix //= 10
        if prefix == 0:
            total += value
    return total


def idempotents_crt(limit):
    """Enumerate zero/one choices over coprime prime-power factors."""
    total = 0
    for n in range(2, limit + 1):
        remaining, factor = n, 2
        powers = []
        while factor * factor <= remaining:
            power = 1
            while remaining % factor == 0:
                remaining //= factor
                power *= factor
            if power > 1:
                powers.append(power)
            factor += 1
        if remaining > 1:
            powers.append(remaining)
        greatest = 0
        for bits in itertools.product((False, True), repeat=len(powers)):
            a = math.prod(power for power, selected in zip(powers, bits) if selected)
            b = n // a
            value = a * pow(a, -1, b) % n
            greatest = max(greatest, value)
        total += greatest
    return total


def idempotents_brute(limit):
    """Search residues directly without using the Chinese remainder theorem."""
    return sum(next(a for a in range(n - 1, -1, -1) if (a * a - a) % n == 0)
               for n in range(2, limit + 1))


def euclid_forward(limit):
    """Count every division in every ordered positive pair directly."""
    total = 0
    for x in range(1, limit + 1):
        for y in range(1, limit + 1):
            a, b = x, y
            while b:
                a, b = b, a % b
                total += 1
    return total


def euclid_reverse(limit):
    """Traverse reverse Euclidean forests and account for reversed pairs."""
    depth_sum = 0
    for root in range(1, limit + 1):
        stack = [(multiple, root, 1) for multiple in range(2 * root, limit + 1, root)]
        while stack:
            x, y, depth = stack.pop()
            depth_sum += depth
            for before in range(x + y, limit + 1, x):
                stack.append((before, x, depth + 1))
    return limit + limit * (limit - 1) // 2 + 2 * depth_sum
