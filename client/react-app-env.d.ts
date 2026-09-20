/// <reference types="react-scripts" />
import type { JSX as ReactJSX } from "react";
import React from "react";

declare global {
    namespace JSX {
        type Element = ReactJSX.Element;
        type ElementClass = ReactJSX.ElementClass;
        type ElementAttributesProperty = ReactJSX.ElementAttributesProperty;
        type ElementChildrenAttribute = ReactJSX.ElementChildrenAttribute;
        type LibraryManagedAttributes<C, P> = ReactJSX.LibraryManagedAttributes<C, P>;
        type IntrinsicAttributes = ReactJSX.IntrinsicAttributes;
        type IntrinsicClassAttributes<T> = ReactJSX.IntrinsicClassAttributes<T>;
        type IntrinsicElements = ReactJSX.IntrinsicElements;
    }
}

declare module "react" {
    function memo<T extends React.ComponentType<any>>(
        c: T,
        areEqual?: (
            prev: Readonly<React.ComponentProps<T>>,
            next: Readonly<React.ComponentProps<T>>,
        ) => boolean,
    ): T;
}
