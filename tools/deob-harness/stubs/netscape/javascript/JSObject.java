package netscape.javascript;

/**
 * Compile-only stand-in for the vendored deob JSObject plus the JDK module
 * class of the same name. The deob tree's own copy lacks a getWindow
 * overload accepting its Client type, and javac resolves this package from
 * module jdk.jsobject before any sourcepath. This file (used via
 * --patch-module, compile only, never at harness runtime) keeps the abstract
 * member shape and adds a catch-all overload.
 */
public abstract class JSObject {
	protected JSObject() {
	}

	public abstract Object call(String methodName, Object... args) throws JSException;

	public abstract Object eval(String s) throws JSException;

	public abstract Object getMember(String name) throws JSException;

	public abstract void setMember(String name, Object value) throws JSException;

	public abstract void removeMember(String name) throws JSException;

	public abstract Object getSlot(int index) throws JSException;

	public abstract void setSlot(int index, Object value) throws JSException;

	public static JSObject getWindow(Object o) throws JSException {
		return null;
	}
}
