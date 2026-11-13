// The product data walls' shader (Product.cs, mesh.productdata): writes each
// fragment's product data as colour, red and green its x and y, blue one
// less its w. The walls' data is their place, 0 to 1 across and up, with w 1,
// so they draw a gradient; a mesh without data reads zero and draws blue.

#import rusty::types::Surface

fn shade(surface: Surface) -> vec4<f32> {
    let data = surface.product_data;
    return vec4<f32>(data.x, data.y, 1.0 - data.w, 1.0);
}
