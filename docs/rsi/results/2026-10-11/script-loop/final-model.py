"""Editable model for the curve-fitting harness.

fit(ts, ys) receives the training inputs and targets and returns a
predict(t) function. This improved model fits a polynomial of degree 8
using a stable least squares approach with Gaussian elimination, which
should better capture the data's curvature and reduce held-out RMSE.
"""

def fit(ts, ys):
    n = len(ts)
    degree = 8
    # Construct Vandermonde matrix for polynomial degree 8
    X = [[t**d for d in range(degree+1)] for t in ts]

    # Compute X^T * X and X^T * y
    XtX = [[0]*(degree+1) for _ in range(degree+1)]
    Xty = [0]*(degree+1)
    for i in range(degree+1):
        for j in range(degree+1):
            XtX[i][j] = sum(X[k][i]*X[k][j] for k in range(n))
        Xty[i] = sum(X[k][i]*ys[k] for k in range(n))

    # Solve linear system XtX * coeffs = Xty using Gaussian elimination
    coeffs = Xty[:]
    for i in range(degree+1):
        # Pivot
        max_row = max(range(i,degree+1), key=lambda r: abs(XtX[r][i]))
        if max_row != i:
            XtX[i], XtX[max_row] = XtX[max_row], XtX[i]
            coeffs[i], coeffs[max_row] = coeffs[max_row], coeffs[i]
        # Normalize pivot row
        pivot = XtX[i][i]
        for j in range(i,degree+1):
            XtX[i][j] /= pivot
        coeffs[i] /= pivot
        # Eliminate below
        for r in range(i+1,degree+1):
            factor = XtX[r][i]
            for c in range(i,degree+1):
                XtX[r][c] -= factor * XtX[i][c]
            coeffs[r] -= factor * coeffs[i]
    # Back substitution
    for i in range(degree,-1,-1):
        for j in range(i+1,degree+1):
            coeffs[i] -= XtX[i][j] * coeffs[j]

    def predict(t):
        return sum(coeffs[d]*t**d for d in range(degree+1))

    return predict
