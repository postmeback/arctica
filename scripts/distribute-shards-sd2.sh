#script for distributing 2 shards to SD card 2

sudo cp /mnt/ramdisk/shards/shard2.txt /home/$USER/shards
sudo rm /mnt/ramdisk/shards/shard2.txt
sudo cp /mnt/ramdisk/shards/shard10.txt /home/$USER/shards
sudo rm /mnt/ramdisk/shards/shard10.txt