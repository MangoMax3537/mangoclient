package gg.mangoclient.mangoconfig;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayDeque;
import java.util.Deque;

/**
 * Clicks per second for either mouse button.
 *
 * Sampled from the HUD hook rather than from an input event, so it counts what
 * the player actually did without another mixin. A click is the moment the
 * button goes down; holding it is one click, not sixty.
 */
public final class ClickCounter {
	private static final long WINDOW_MS = 1000;

	private static final Deque<Long> left = new ArrayDeque<>();
	private static final Deque<Long> right = new ArrayDeque<>();
	private static boolean leftWasDown;
	private static boolean rightWasDown;

	private ClickCounter() {
	}

	public static void tick() {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc == null || mc.getWindow() == null) return;

		long handle = mc.getWindow().getHandle();
		boolean leftDown = GLFW.glfwGetMouseButton(handle, GLFW.GLFW_MOUSE_BUTTON_LEFT) == GLFW.GLFW_PRESS;
		boolean rightDown = GLFW.glfwGetMouseButton(handle, GLFW.GLFW_MOUSE_BUTTON_RIGHT) == GLFW.GLFW_PRESS;

		long now = System.currentTimeMillis();
		if (leftDown && !leftWasDown) left.addLast(now);
		if (rightDown && !rightWasDown) right.addLast(now);
		leftWasDown = leftDown;
		rightWasDown = rightDown;

		prune(left, now);
		prune(right, now);
	}

	private static void prune(Deque<Long> q, long now) {
		while (!q.isEmpty() && now - q.peekFirst() > WINDOW_MS) q.removeFirst();
	}

	public static int leftCps() {
		return left.size();
	}

	public static int rightCps() {
		return right.size();
	}

	/** Whether a key is held, for the keystrokes module. */
	public static boolean isDown(MinecraftClient mc, int key) {
		return InputUtil.isKeyPressed(mc.getWindow(), key);
	}
}
