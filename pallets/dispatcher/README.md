# Dispatcher Pallet
This pallet enables specific OpenGov tracks to dispatch Runtime calls as predefined origins.  

The pallet supports the following dispatchables:
* `dispatch_as_treasury` - allows the `Treasury` track to dispatch calls as the Treasury account on Hydration (`7L53bUTBopuwFt3mKUfmkzgGLayYa1Yvn1hAg9v5UMrQzTfh`)
* `dispatch_as_aave_manager` - allows the `AaveManager` track to dispatch calls as the Money Market authority on Hydration (`add addr`)
