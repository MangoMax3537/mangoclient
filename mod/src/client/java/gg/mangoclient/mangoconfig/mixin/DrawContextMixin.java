package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.gui.DrawContext;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Where the tab list, the scoreboard and F3 get their see-through backgrounds
 * and F3 its text colour.
 *
 * Each of those draws through these few DrawContext calls in every version, so
 * the colour is changed on the way in while one of them is drawing (see
 * Visuals.scope) - which also catches the fills vanilla makes from inside a
 * lambda. Outside those scopes this is one field read per call.
 */
@Mixin(DrawContext.class)
public class DrawContextMixin {
	@ModifyVariable(method = "fill(IIIII)V", at = @At("HEAD"), argsOnly = true, ordinal = 4)
	private int mangoconfig$fillColour(int colour) {
		return Visuals.scope == Visuals.Scope.NONE ? colour : Visuals.fill(colour);
	}

	// All three drawText overloads (String, Text, OrderedText) take x, y and the
	// colour as their only ints, and the shadow as their only boolean.
	@ModifyVariable(method = "drawText", at = @At("HEAD"), argsOnly = true, ordinal = 2)
	private int mangoconfig$textColour(int colour) {
		return Visuals.scope == Visuals.Scope.NONE ? colour : Visuals.text(colour);
	}

	@ModifyVariable(method = "drawText", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private boolean mangoconfig$textShadow(boolean shadow) {
		return Visuals.scope == Visuals.Scope.NONE ? shadow : Visuals.shadow(shadow);
	}
}
