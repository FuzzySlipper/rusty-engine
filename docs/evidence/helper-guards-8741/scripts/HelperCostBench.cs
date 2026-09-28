using System.Diagnostics;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

// #8741 helper cost: inventory grants on a store with many inventories, and entity
// batch writes on a store with many entities.
const int Owners = 2000;
const int StacksPerOwner = 10;
const int Operations = 2000;

var item = new ItemDefinition(ItemDefinitionId.Parse("coin"), ItemKind.Fungible, 1_000_000);
var store = new InventoryStore();
for (int owner = 1; owner <= Owners; owner++)
{
    store.RegisterInventory(new InventoryState(new EntityId((ulong)owner)));
    for (int stack = 0; stack < StacksPerOwner; stack++)
        store.Grant(new EntityId((ulong)owner), item, InventoryStackId.Parse($"s{stack}"), 1);
}
var watch = Stopwatch.StartNew();
for (int index = 0; index < Operations; index++)
    store.Grant(new EntityId((ulong)(index % Owners + 1)), item, InventoryStackId.Parse("s0"), 1);
double grantUs = watch.Elapsed.TotalMicroseconds / Operations;

watch.Restart();
for (int index = 0; index < Operations / 10; index++)
{
    using InventoryEdit edit = store.Prepare();
    edit.Grant(new EntityId(1), item, InventoryStackId.Parse("s1"), 1);
    edit.Grant(new EntityId(2), item, InventoryStackId.Parse("s1"), 1);
    edit.Publish();
}
double editUs = watch.Elapsed.TotalMicroseconds / (Operations / 10);

var positions = ComponentType<Vector3>.Create(ProductComponentKeys.Create(1));
using var entities = new EntityStore([EngineComponentTypes.Transform, positions]);
var ids = new EntityId[5000];
for (int index = 0; index < ids.Length; index++)
{
    ids[index] = entities.Create();
    entities.Set(ids[index], positions, new Vector3(index));
}
watch.Restart();
for (int index = 0; index < Operations; index++)
    entities.Commit(new EntityBatch().Set(ids[index % ids.Length], positions, new Vector3(index)));
double batchUs = watch.Elapsed.TotalMicroseconds / Operations;

Console.WriteLine($"{{\"inventoryGrantUs\":{grantUs:F1},\"inventoryEditUs\":{editUs:F1},\"entityBatchUs\":{batchUs:F1},\"owners\":{Owners},\"stacksPerOwner\":{StacksPerOwner},\"entities\":{ids.Length}}}");
