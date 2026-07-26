pub(super) fn compose_trs(translation: [f64; 3], rotation: [f64; 4], scale: [f64; 3]) -> [f64; 16] {
    let [x, y, z, w] = rotation;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    [
        (1.0 - 2.0 * (yy + zz)) * scale[0],
        (2.0 * (xy + wz)) * scale[0],
        (2.0 * (xz - wy)) * scale[0],
        0.0,
        (2.0 * (xy - wz)) * scale[1],
        (1.0 - 2.0 * (xx + zz)) * scale[1],
        (2.0 * (yz + wx)) * scale[1],
        0.0,
        (2.0 * (xz + wy)) * scale[2],
        (2.0 * (yz - wx)) * scale[2],
        (1.0 - 2.0 * (xx + yy)) * scale[2],
        0.0,
        translation[0],
        translation[1],
        translation[2],
        1.0,
    ]
}

pub(super) fn invert_affine(matrix: [f64; 16]) -> Option<[f64; 16]> {
    if matrix.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let a = matrix[0];
    let b = matrix[4];
    let c = matrix[8];
    let d = matrix[1];
    let e = matrix[5];
    let f = matrix[9];
    let g = matrix[2];
    let h = matrix[6];
    let i = matrix[10];
    let determinant = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
        return None;
    }
    let inverse_determinant = 1.0 / determinant;
    let inverse = [
        (e * i - f * h) * inverse_determinant,
        (f * g - d * i) * inverse_determinant,
        (d * h - e * g) * inverse_determinant,
        0.0,
        (c * h - b * i) * inverse_determinant,
        (a * i - c * g) * inverse_determinant,
        (b * g - a * h) * inverse_determinant,
        0.0,
        (b * f - c * e) * inverse_determinant,
        (c * d - a * f) * inverse_determinant,
        (a * e - b * d) * inverse_determinant,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ];
    let translation = [matrix[12], matrix[13], matrix[14]];
    let inverse_translation = transform_vector(inverse, translation).map(|value| -value);
    let mut result = inverse;
    result[12] = inverse_translation[0];
    result[13] = inverse_translation[1];
    result[14] = inverse_translation[2];
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

fn transform_vector(matrix: [f64; 16], vector: [f64; 3]) -> [f64; 3] {
    [
        matrix[0] * vector[0] + matrix[4] * vector[1] + matrix[8] * vector[2],
        matrix[1] * vector[0] + matrix[5] * vector[1] + matrix[9] * vector[2],
        matrix[2] * vector[0] + matrix[6] * vector[1] + matrix[10] * vector[2],
    ]
}
