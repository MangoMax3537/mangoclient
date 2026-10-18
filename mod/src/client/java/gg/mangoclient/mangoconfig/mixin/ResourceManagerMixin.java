package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.BundledAssets;
import net.minecraft.resource.LifecycledResourceManagerImpl;
import net.minecraft.resource.Resource;
import net.minecraft.util.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import java.util.function.Predicate;

/**
 * Lets the game find this jar's own assets without Fabric API (see
 * BundledAssets). Only ever adds what is missing: anything a pack provides
 * under the same name still wins.
 */
@Mixin(LifecycledResourceManagerImpl.class)
public class ResourceManagerMixin {
	@Inject(method = "getResource", at = @At("RETURN"), cancellable = true)
	private void mangoconfig$bundled(Identifier id, CallbackInfoReturnable<Optional<Resource>> cir) {
		if (cir.getReturnValue().isEmpty()) {
			Optional<Resource> bundled = BundledAssets.get(id);
			if (bundled.isPresent()) cir.setReturnValue(bundled);
		}
	}

	@Inject(method = "findResources", at = @At("RETURN"), cancellable = true)
	private void mangoconfig$bundledListing(String startingPath, Predicate<Identifier> allowedPathPredicate,
			CallbackInfoReturnable<Map<Identifier, Resource>> cir) {
		Map<Identifier, Resource> bundled = BundledAssets.find(startingPath, allowedPathPredicate);
		if (bundled.isEmpty()) return;
		Map<Identifier, Resource> merged = new HashMap<>(bundled);
		merged.putAll(cir.getReturnValue());
		cir.setReturnValue(merged);
	}
}
