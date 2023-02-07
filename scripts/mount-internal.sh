#the below internal drive configurations assume a default ubuntu install on the internal disk without any 
#custom partitioning
#mount internal drive if nvme
udisksctl mount --block-device /dev/nvme0n1p2
#mount internal drive if SATA
udisksctl mount --block-device /dev/sda2
#remove stale symlinks
sudo chmod 777 /home/$USER/.bitcoin
sudo unlink /home/$USER/.bitcoin/chainstate
sudo unlink /home/$USER/.bitcoin/blocks

# UUID=$(echo $(blkid) | cut -d '"' -f 2)

#obtain Host User and UUID mounted by udisksctl
PLACEHOLDER=$(ls /media/$USER)
strarr=($PLACEHOLDER)

for val in "${strarr[@]}";
do
    if	[ "$val" = "writable" ]
    then
            echo pass
    elif  [ "$val" = "CDROM" ]
    then
            echo pass
    else
            echo $val
            UUID=$val
    fi

done
HOST_USER=$(ls /media/$USER/$UUID/home)
#open file permissions for local host
sudo chmod 777 /media/ubuntu/$UUID/home/$HOST_USER

#remove stale bitcoin dirs if they exist
sudo rm -r /home/$USER/.bitcoin/chainstate
sudo rm -r /home/$USER/.bitcoin/blocks

#make local internal bitcoin dotfile
sudo mkdir --parents /media/ubuntu/$UUID/home/$HOST_USER/.bitcoin/blocks /media/ubuntu/$UUID/home/$HOST_USER/.bitcoin/chainstate	

#create symlinks
ln -s /media/$USER/$UUID/home/$HOST_USER/.bitcoin/chainstate /home/$USER/.bitcoin
ln -s /media/$USER/$UUID/home/$HOST_USER/.bitcoin/blocks /home/$USER/.bitcoin

sudo chmod -R 777 /media/ubuntu/$UUID/home/$HOST_USER/.bitcoin

