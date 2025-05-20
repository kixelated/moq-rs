import assert from "node:assert";
import test from "node:test";
import { TrackProducer } from "./track";

test("track clone", async () => {
	const producer = new TrackProducer("test", 0);

	// Clone the reader before we append any groups
	const consumerA = producer.consume();
	const consumerB = consumerA.clone();

	const group1 = producer.appendGroup();

	// Clone the reader after we appended that group; we still get it.
	const consumerC = consumerA.clone();

	const group1A = await consumerA.nextGroup();
	const group1B = await consumerB.nextGroup();
	const group1C = await consumerC.nextGroup();

	assert.strictEqual(group1A?.id, group1.id);
	assert.strictEqual(group1B?.id, group1.id);
	assert.strictEqual(group1C?.id, group1.id);

	// Append a new group, everybody gets it
	const group2 = producer.appendGroup();

	const group2A = await consumerA.nextGroup();
	const group2B = await consumerB.nextGroup();
	const group2C = await consumerC.nextGroup();

	assert.strictEqual(group2A?.id, group2.id);
	assert.strictEqual(group2B?.id, group2.id);
	assert.strictEqual(group2C?.id, group2.id);

	// Clone the reader after we appended that group.
	// This new reader gets the most recent group but that's it.
	const consumerD = consumerA.clone();

	const group2D = await consumerD.nextGroup();
	assert.strictEqual(group2D?.id, group2.id);

	// Everybody gets the new group
	const group3 = producer.appendGroup();

	const group3A = await consumerA.nextGroup();
	const group3B = await consumerB.nextGroup();
	const group3C = await consumerC.nextGroup();
	const group3D = await consumerD.nextGroup();

	assert.strictEqual(group3A?.id, group3.id);
	assert.strictEqual(group3B?.id, group3.id);
	assert.strictEqual(group3C?.id, group3.id);
	assert.strictEqual(group3D?.id, group3.id);

	// It's okay to close readers.
	consumerA.close();
	consumerB.close();

	const group4 = producer.appendGroup();

	const group4A = await consumerA.nextGroup();
	const group4B = await consumerB.nextGroup();
	const group4C = await consumerC.nextGroup();
	const group4D = await consumerD.nextGroup();

	assert.strictEqual(group4A?.id, undefined);
	assert.strictEqual(group4B?.id, undefined);
	assert.strictEqual(group4C?.id, group4.id);
	assert.strictEqual(group4D?.id, group4.id);

	const consumerE = consumerC.clone();
	const group4E = await consumerE.nextGroup();
	assert.strictEqual(group4E?.id, group4.id);
});

test("track group cloning", async () => {
	const producer = new TrackProducer("test", 0);
	const consumerA = producer.consume();
	const consumerB = consumerA.clone();

	// Make sure both readers get separate copies of the groups.
	const group = producer.appendGroup();
	group.writeFrame(new Uint8Array([1]));
	group.writeFrame(new Uint8Array([2]));
	group.writeFrame(new Uint8Array([3]));

	const groupA = await consumerA.nextGroup();
	const groupB = await consumerB.nextGroup();

	assert.strictEqual(groupA?.id, group.id);
	assert.strictEqual(groupB?.id, group.id);

	const frame1A = await groupA.readFrame();
	const frame1B = await groupB.readFrame();

	assert.deepEqual(frame1A, new Uint8Array([1]));
	assert.deepEqual(frame1B, new Uint8Array([1]));

	const frame2A = await groupA.readFrame();
	groupA.close(); // closing doesn't impact the other reader
	const frame2B = await groupB.readFrame();

	assert.deepEqual(frame2A, new Uint8Array([2]));
	assert.deepEqual(frame2B, new Uint8Array([2]));

	const frame3A = await groupA.readFrame();
	const frame3B = await groupB.readFrame();

	assert.deepEqual(frame3A, undefined);
	assert.deepEqual(frame3B, new Uint8Array([3]));
});
