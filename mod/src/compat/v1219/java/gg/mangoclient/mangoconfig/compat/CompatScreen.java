package gg.mangoclient.mangoconfig.compat;

import net.minecraft.client.gui.Click;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.input.CharInput;
import net.minecraft.client.input.KeyInput;
import net.minecraft.text.Text;

/**
 * A Screen with version-neutral input hooks.
 *
 * 1.21.9 rewrote every input override around Click/KeyInput/CharInput records;
 * before that they were loose doubles and ints. The screens implement the
 * on*() hooks once, and each compat family owns this translation instead.
 */
public abstract class CompatScreen extends Screen {
	protected CompatScreen(Text title) {
		super(title);
	}

	protected boolean onMouseDown(double x, double y) {
		return false;
	}

	protected boolean onMouseUp(double x, double y) {
		return false;
	}

	protected boolean onMouseDrag(double x, double y, double dx, double dy) {
		return false;
	}

	protected boolean onKeyDown(int key) {
		return false;
	}

	protected boolean onCharTyped(char chr) {
		return false;
	}

	@Override
	public boolean mouseClicked(Click click, boolean doubled) {
		return onMouseDown(click.x(), click.y()) || super.mouseClicked(click, doubled);
	}

	@Override
	public boolean mouseReleased(Click click) {
		return onMouseUp(click.x(), click.y()) || super.mouseReleased(click);
	}

	@Override
	public boolean mouseDragged(Click click, double dx, double dy) {
		return onMouseDrag(click.x(), click.y(), dx, dy) || super.mouseDragged(click, dx, dy);
	}

	@Override
	public boolean keyPressed(KeyInput input) {
		return onKeyDown(input.key()) || super.keyPressed(input);
	}

	@Override
	public boolean charTyped(CharInput input) {
		if (input.isValidChar()) {
			String s = input.asString();
			if (!s.isEmpty() && onCharTyped(s.charAt(0))) return true;
		}
		return super.charTyped(input);
	}
}
