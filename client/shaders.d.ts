declare module "*.glsl" {
    const value: string;
    export default value;
}

declare module "*.wgsl?source" {
    const value: string;
    export default value;
}

declare module "*.css" {
    const content: { readonly [className: string]: string };
    export default content;
}
