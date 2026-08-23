package gg.mangoclient.mangoconfig.compat;

import net.minecraft.util.StringHelper;
import net.minecraft.client.gui.screen.Screen;
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
	public boolean mouseClicked(double mouseX, double mouseY, int button) {
		return onMouseDown(mouseX, mouseY) || super.mouseClicked(mouseX, mouseY, button);
	}

	@Override
	public boolean mouseReleased(double mouseX, double mouseY, int button) {
		return onMouseUp(mouseX, mouseY) || super.mouseReleased(mouseX, mouseY, button);
	}

	@Override
	public boolean mouseDragged(double mouseX, double mouseY, int button, double dx, double dy) {
		return onMouseDrag(mouseX, mouseY, dx, dy) || super.mouseDragged(mouseX, mouseY, button, dx, dy);
	}

	@Override
	public boolean keyPressed(int key, int scancode, int modifiers) {
		return onKeyDown(key) || super.keyPressed(key, scancode, modifiers);
	}

	@Override
	public boolean charTyped(char chr, int modifiers) {
		return (StringHelper.isValidChar(chr) && onCharTyped(chr)) || super.charTyped(chr, modifiers);
	}
}
